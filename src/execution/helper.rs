// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use crate::{
    filesystem::MountPoint,
    isolation::{
        AssignedRoots, CommandDevices, CommandReceiver, GatedCommandSetup, GatedRootSetup, Mapped,
        PrivateMountNamespace, UserNamespace,
    },
    process::Supervision,
    step::SetupCutoff,
};
use std::{
    ffi::{OsStr, OsString},
    io,
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        process::ExitStatusExt,
    },
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    time::{Duration, Instant},
};

const SELECTOR: &str = "--conglobate-helper";
const MAX_ARGUMENT: usize = 128 * 1024;

/// The trusted executable which dispatches HelperInvocation before public CLI
/// parsing. It is launched before entering the image root; no executable is
/// copied into that root. The deployment owns this executable and its libraries.
#[derive(Debug)]
pub struct HelperProgram(PathBuf);
impl HelperProgram {
    pub fn current() -> io::Result<Self> {
        std::env::current_exe().map(Self)
    }
    fn command(&self, invocation: HelperInvocation) -> io::Result<Command> {
        let encoded = invocation.encode();
        if encoded.len() > MAX_ARGUMENT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "helper launch input exceeds argv limit",
            ));
        }
        let mut command = Command::new(&self.0);
        command.arg(SELECTOR).arg(encoded).env_clear();
        // Helper and image-command output are build logs. Keep the public CLI
        // stdout separate; stdin carries the private structured control channel.
        command.stdout(std::process::Stdio::from(
            rustix::stdio::stderr().try_clone_to_owned()?,
        ));
        Ok(command)
    }
    pub(crate) fn formatter(
        &self,
        executable: PathBuf,
        arguments: Vec<OsString>,
        cutoff: SetupCutoff,
    ) -> io::Result<Command> {
        self.command(HelperInvocation {
            cutoff,
            role: Role::Formatter {
                executable,
                arguments,
            },
        })
    }
    pub fn namespace_creator(&self, cutoff: SetupCutoff) -> io::Result<Command> {
        self.command(HelperInvocation {
            cutoff,
            role: Role::Seed,
        })
    }
    pub(crate) fn network_namespace_creator(&self, cutoff: SetupCutoff) -> io::Result<Command> {
        self.command(HelperInvocation {
            cutoff,
            role: Role::NetworkSeed,
        })
    }
    pub(crate) fn network_configure(
        &self,
        configuration: crate::network::LinkConfiguration,
        cutoff: SetupCutoff,
    ) -> io::Result<Command> {
        self.command(HelperInvocation {
            cutoff,
            role: Role::NetworkConfigure(configuration),
        })
    }
    pub fn receiver(
        &self,
        script: String,
        cutoff: SetupCutoff,
        uid: u32,
        gid: u32,
        mount_parent: &Path,
    ) -> io::Result<Command> {
        let role = Role::receiver(script, uid, gid, mount_parent.to_owned())?;
        self.command(HelperInvocation { cutoff, role })
    }
}
#[derive(Debug)]
enum Role {
    Seed,
    NetworkSeed,
    NetworkConfigure(crate::network::LinkConfiguration),
    Receiver {
        script: String,
        uid: u32,
        gid: u32,
        parent: PathBuf,
    },
    Namespaces(String, AssignedRoots),
    Root(String, AssignedRoots),
    Formatter {
        executable: PathBuf,
        arguments: Vec<OsString>,
    },
}
impl Role {
    fn receiver(script: String, uid: u32, gid: u32, parent: PathBuf) -> io::Result<Self> {
        if uid == 0
            || gid == 0
            || !parent.is_absolute()
            || parent.as_os_str().as_bytes().contains(&0)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "receiver requires non-host-root IDs and an absolute trusted mount parent",
            ));
        }
        Ok(Self::Receiver {
            script,
            uid,
            gid,
            parent,
        })
    }
}
/// Immutable, versioned private launch input. It contains no mode selecting a
/// fixture fault. Possession is not authorization: only the supervisor launches
/// these roles and supplies their private startup descriptors/containment.
#[derive(Debug)]
pub struct HelperInvocation {
    cutoff: SetupCutoff,
    role: Role,
}
impl HelperInvocation {
    /// Recognize the internal selector and decode its launch description.
    /// Returns None for public CLI arguments. Parsing is not admission authority;
    /// execution still validates the trusted helper's channels and environment.
    ///
    /// # Errors
    /// Rejects a selected helper invocation with the wrong argument count,
    /// non-UTF-8/invalid hexadecimal CBOR, or an invalid launch description.
    pub fn parse(arguments: &[OsString]) -> io::Result<Option<Self>> {
        if arguments.first().map(OsString::as_os_str) != Some(OsStr::new(SELECTOR)) {
            return Ok(None);
        }
        if arguments.len() != 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "helper requires one encoded launch input",
            ));
        }
        let text = arguments[1].to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "helper input must be hexadecimal CBOR",
            )
        })?;
        Self::decode(text).map(Some)
    }
    fn encode(&self) -> String {
        let mut e = minicbor::Encoder::new(Vec::new());
        let (length, tag) = match self.role {
            Role::Seed => (3, 0),
            Role::NetworkSeed => (3, 5),
            Role::NetworkConfigure(_) => (4, 6),
            Role::Receiver { .. } => (7, 1),
            Role::Namespaces(_, _) => (5, 2),
            Role::Root(_, _) => (5, 3),
            Role::Formatter { .. } => (5, 4),
        };
        e.array(length)
            .expect("Vec writer")
            .u8(1)
            .expect("Vec writer")
            .u8(tag)
            .expect("Vec writer")
            .str(&self.cutoff.to_argument())
            .expect("Vec writer");
        match &self.role {
            Role::Seed | Role::NetworkSeed => {}
            Role::NetworkConfigure(configuration) => {
                e.bytes(&configuration.encode()).expect("Vec writer");
            }
            Role::Receiver {
                script,
                uid,
                gid,
                parent,
            } => {
                e.str(script)
                    .expect("Vec writer")
                    .u32(*uid)
                    .expect("Vec writer")
                    .u32(*gid)
                    .expect("Vec writer")
                    .bytes(parent.as_os_str().as_bytes())
                    .expect("Vec writer");
            }
            Role::Formatter {
                executable,
                arguments,
            } => {
                e.bytes(executable.as_os_str().as_bytes())
                    .expect("Vec writer");
                e.array(arguments.len() as u64).expect("Vec writer");
                for arg in arguments {
                    e.bytes(arg.as_bytes()).expect("Vec writer");
                }
            }
            Role::Namespaces(script, roots) | Role::Root(script, roots) => {
                e.str(script)
                    .expect("Vec writer")
                    .u8(roots.tag())
                    .expect("Vec writer");
            }
        }
        hex::encode(e.into_writer())
    }
    fn decode(text: &str) -> io::Result<Self> {
        if text.len() > MAX_ARGUMENT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "oversized helper input",
            ));
        }
        let bytes = hex::decode(text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let mut d = minicbor::Decoder::new(&bytes);
        let parse = (|| -> Result<_, Box<dyn std::error::Error>> {
            let length = d.array()?;
            if d.u8()? != 1 {
                return Err("unknown helper version".into());
            }
            let tag = d.u8()?;
            let cutoff = SetupCutoff::from_argument(d.str()?)?;
            let role = match (length, tag) {
                (Some(3), 0) => Role::Seed,
                (Some(3), 5) => Role::NetworkSeed,
                (Some(4), 6) => {
                    Role::NetworkConfigure(crate::network::LinkConfiguration::decode(d.bytes()?)?)
                }
                (Some(7), 1) => Role::receiver(
                    d.str()?.to_owned(),
                    d.u32()?,
                    d.u32()?,
                    PathBuf::from(OsString::from_vec(d.bytes()?.to_vec())),
                )?,
                (Some(5), 2) => Role::Namespaces(d.str()?.to_owned(), d.u8()?.try_into()?),
                (Some(5), 3) => Role::Root(d.str()?.to_owned(), d.u8()?.try_into()?),
                (Some(5), 4) => {
                    let executable = PathBuf::from(OsString::from_vec(d.bytes()?.to_vec()));
                    let count = d.array()?.ok_or("indefinite formatter arguments")?;
                    if count > 128 {
                        return Err("too many formatter arguments".into());
                    }
                    let arguments = (0..count)
                        .map(|_| d.bytes().map(|b| OsString::from_vec(b.to_vec())))
                        .collect::<Result<Vec<_>, _>>()?;
                    Role::Formatter {
                        executable,
                        arguments,
                    }
                }

                _ => return Err("invalid helper role/shape".into()),
            };
            Ok(Self { cutoff, role })
        })()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        if d.position() != bytes.len() || parse.encode() != text {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "noncanonical helper input",
            ));
        }
        Ok(parse)
    }
    /// Must be called in the dedicated helper process. Every path exits; no
    /// caller can continue supervisor work after partial namespace/root entry.
    pub fn execute(self, program: &HelperProgram) -> ! {
        let result = self.run(program);
        match result {
            Ok(status) => {
                std::process::exit(status.code().unwrap_or(128 + status.signal().unwrap_or(1)))
            }
            Err(error) => {
                // Render before writing: cancellation can follow channel loss,
                // so avoid several separate writes for one failure diagnostic.
                use std::io::Write;
                let message = format!("trusted command helper: {error}\n");
                let _ = io::stderr().lock().write_all(message.as_bytes());
                std::process::exit(1);
            }
        }
    }
    fn run(self, program: &HelperProgram) -> Result<ExitStatus, Box<dyn std::error::Error>> {
        let deadline = self.cutoff.deadline()?;
        match self.role {
            Role::Formatter {
                executable,
                arguments,
            } => {
                use std::os::unix::process::CommandExt;
                // No restoration: every return from this dedicated role exits.
                nix::sched::unshare(
                    nix::sched::CloneFlags::CLONE_NEWNS
                        | nix::sched::CloneFlags::CLONE_NEWNET
                        | nix::sched::CloneFlags::CLONE_NEWIPC
                        | nix::sched::CloneFlags::CLONE_NEWUTS,
                )?;
                rustix::mount::mount_change(
                    "/",
                    rustix::mount::MountPropagationFlags::PRIVATE
                        | rustix::mount::MountPropagationFlags::REC,
                )?;
                crate::formatter::FormatterInput::from_stdin()?.enter()?;
                self.cutoff.deadline()?;
                Err(Command::new(executable)
                    .args(arguments)
                    .env_clear()
                    .env("MKE2FS_CONFIG", "/etc/mke2fs.conf")
                    .exec()
                    .into())
            }
            Role::NetworkConfigure(configuration) => {
                configuration.serve(deadline)?;
                Ok(ExitStatus::from_raw(0))
            }
            Role::NetworkSeed => {
                crate::network::NetworkNamespace::serve(deadline)?;
                Ok(ExitStatus::from_raw(0))
            }
            Role::Seed => {
                UserNamespace::<Mapped>::serve(deadline)?;
                Ok(ExitStatus::from_raw(0))
            }
            Role::Namespaces(script, roots) => {
                let setup = GatedCommandSetup::from_input()?;
                let root = program.command(Self {
                    cutoff: self.cutoff,
                    role: Role::Root(script, roots),
                })?;
                Ok(setup.execute(root, self.cutoff)?)
            }
            Role::Root(script, roots) => {
                let command = GatedRootSetup::from_input()?.restrict(&script, self.cutoff)?;
                // Write while the readiness endpoint is still owned: the
                // receiver must be able to drain this diagnostic before loss.
                roots.verify_labels().inspect_err(|error| {
                    eprintln!("restricted labeling validation: {error}");
                })?;
                self.cutoff.deadline()?;
                match command.start()? {}
            }
            Role::Receiver {
                script,
                uid,
                gid,
                parent,
            } => {
                let channel = Supervision::from_stdin()?
                    .exchange_until(deadline)?
                    .join_domain_until(deadline)?;
                let input = channel
                    .receive_volume_until(deadline)
                    .map_err(|error| format!("receive command volume: {error}"))?;
                let roots = input.assigned_roots();
                let scope = PrivateMountNamespace::enter()?;
                let (view, setup, resolver) = input
                    .prepare(&scope, CommandDevices::create(uid, gid)?)
                    .map_err(|error| format!("prepare command volume: {error}"))?;
                self.cutoff.deadline()?;
                let view = view
                    .attach(MountPoint::open(&parent)?)
                    .map_err(|e| e.to_string())?;
                let entry = program.command(Self {
                    cutoff: self.cutoff,
                    role: Role::Namespaces(script, roots),
                })?;
                // Borrowed mount errors become helper diagnostics immediately
                // before guaranteed process exit. Parent ownership is separate.
                let completion = CommandReceiver::new(channel, view, setup)
                    .execute(entry, self.cutoff)
                    .map_err(|e| e.to_string())?;
                let (channel, view, status) = completion.into_parts();
                // Namespace init has exited and its namespace descendants are
                // dead before the trusted namespace helper can be reaped. Its
                // resolver mount namespace is now gone; remove only our pinned
                // scaffolding before closing this root view. Failed cleanup
                // prevents the helper from reporting successful completion.
                if let Some(resolver) = resolver {
                    resolver.release()?;
                }
                view.close().map_err(|e| e.to_string())?;
                scope.restore().map_err(|e| e.to_string())?;
                channel.report_cleanup_until(Instant::now() + Duration::from_secs(30))?;
                channel.wait_for_loss_until(Instant::now() + Duration::from_secs(30))?;
                Ok(status)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::SetupBudget;

    #[test]
    fn launch_roles_round_trip_without_renewing_cutoff() {
        let cutoff = SetupBudget::begin().cutoff().unwrap();
        let roles = [
            Role::Seed,
            Role::NetworkSeed,
            Role::receiver(
                "printf '%s' hello".into(),
                100000,
                100000,
                PathBuf::from(OsString::from_vec(b"/work/\xff".to_vec())),
            )
            .unwrap(),
            Role::Namespaces("exit 37".into(), AssignedRoots::Root),
            Role::Namespaces("exit 37".into(), AssignedRoots::Bootstrap),
            Role::Root("exit 37".into(), AssignedRoots::Root),
            Role::Root("exit 37".into(), AssignedRoots::Bootstrap),
            Role::Formatter {
                executable: "/usr/bin/mke2fs".into(),
                arguments: vec!["-L".into(), "literal ; $name".into()],
            },
        ];
        for role in roles {
            let invocation = HelperInvocation { cutoff, role };
            let encoded = invocation.encode();
            let decoded = HelperInvocation::decode(&encoded).unwrap();
            assert_eq!(decoded.encode(), encoded);
            assert_eq!(decoded.cutoff.to_argument(), cutoff.to_argument());
            assert!(HelperInvocation::decode(&(encoded.clone() + "00")).is_err());
        }
    }

    #[test]
    fn malformed_launch_input_is_rejected_before_dispatch() {
        assert!(HelperInvocation::parse(&[]).unwrap().is_none());
        assert!(
            HelperInvocation::parse(&["build".into()])
                .unwrap()
                .is_none()
        );
        assert!(HelperInvocation::parse(&[SELECTOR.into()]).is_err());
        assert!(HelperInvocation::parse(&[SELECTOR.into(), "00".into(), "extra".into()]).is_err());
        for text in ["", "ff", "83020060", "83010960", "not hex"] {
            assert!(HelperInvocation::decode(text).is_err());
        }
        assert!(HelperInvocation::decode(&"0".repeat(MAX_ARGUMENT + 1)).is_err());
        for (uid, gid, path) in [(0, 1, "/work"), (1, 0, "/work"), (1, 1, "relative")] {
            assert!(Role::receiver(String::new(), uid, gid, path.into()).is_err());
        }
    }

    #[test]
    fn receiver_argument_limit_applies_before_spawn() {
        let program = HelperProgram::current().unwrap();
        assert!(
            program
                .receiver(
                    "a".repeat(MAX_ARGUMENT),
                    SetupBudget::begin().cutoff().unwrap(),
                    100000,
                    100000,
                    Path::new("/work"),
                )
                .is_err()
        );
    }
}
