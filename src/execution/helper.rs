// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use crate::{
    filesystem::MountPoint,
    isolation::{
        CommandDevices, CommandReceiver, GatedCommandSetup, GatedRootSetup, Mapped,
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
        Ok(command)
    }
    pub fn namespace_creator(&self, cutoff: SetupCutoff) -> io::Result<Command> {
        self.command(HelperInvocation {
            cutoff,
            role: Role::Seed,
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
    Receiver {
        script: String,
        uid: u32,
        gid: u32,
        parent: PathBuf,
    },
    Namespaces(String),
    Root(String),
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
            Role::Receiver { .. } => (7, 1),
            Role::Namespaces(_) => (4, 2),
            Role::Root(_) => (4, 3),
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
            Role::Seed => {}
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
            Role::Namespaces(script) | Role::Root(script) => {
                e.str(script).expect("Vec writer");
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
                (Some(7), 1) => Role::receiver(
                    d.str()?.to_owned(),
                    d.u32()?,
                    d.u32()?,
                    PathBuf::from(OsString::from_vec(d.bytes()?.to_vec())),
                )?,
                (Some(4), 2) => Role::Namespaces(d.str()?.to_owned()),
                (Some(4), 3) => Role::Root(d.str()?.to_owned()),
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
                eprintln!("trusted command helper: {error}");
                std::process::exit(1);
            }
        }
    }
    fn run(self, program: &HelperProgram) -> Result<ExitStatus, Box<dyn std::error::Error>> {
        let deadline = self.cutoff.deadline()?;
        match self.role {
            Role::Seed => {
                UserNamespace::<Mapped>::serve(deadline)?;
                Ok(ExitStatus::from_raw(0))
            }
            Role::Namespaces(script) => {
                let setup = GatedCommandSetup::from_input()?;
                let root = program.command(Self {
                    cutoff: self.cutoff,
                    role: Role::Root(script),
                })?;
                Ok(setup.execute(root, self.cutoff)?)
            }
            Role::Root(script) => {
                let command = GatedRootSetup::from_input()?.restrict(&script, self.cutoff)?;
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
                let input = channel.receive_volume_until(deadline)?;
                let scope = PrivateMountNamespace::enter()?;
                let (view, setup) = input.prepare(&scope, CommandDevices::create(uid, gid)?)?;
                self.cutoff.deadline()?;
                let view = view
                    .attach(MountPoint::create(&parent)?)
                    .map_err(|e| e.to_string())?;
                let entry = program.command(Self {
                    cutoff: self.cutoff,
                    role: Role::Namespaces(script),
                })?;
                // Borrowed mount errors become helper diagnostics immediately
                // before guaranteed process exit. Parent ownership is separate.
                let completion = CommandReceiver::new(channel, view, setup)
                    .execute(entry, self.cutoff)
                    .map_err(|e| e.to_string())?;
                let (channel, view, status) = completion.into_parts();
                view.close().map_err(|e| e.to_string())?;
                scope.restore().map_err(|e| e.to_string())?;
                channel
                    .report_completion_until(status, Instant::now() + Duration::from_secs(30))?;
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
            Role::receiver(
                "printf '%s' hello".into(),
                100000,
                100000,
                PathBuf::from(OsString::from_vec(b"/work/\xff".to_vec())),
            )
            .unwrap(),
            Role::Namespaces("exit 37".into()),
            Role::Root("exit 37".into()),
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
