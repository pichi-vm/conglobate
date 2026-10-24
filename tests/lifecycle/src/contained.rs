// SPDX-License-Identifier: Apache-2.0
use conglobate::{
    anchor::RecoveryDirectory,
    domain::{CommandDomain, helper::HelperLaunchFailure},
    process::{ChildReaper, LaunchFailure, Supervision},
};
use std::{
    fs,
    io::{Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixDatagram, process::ExitStatusExt},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy)]
pub(super) enum HelperMode {
    RejectJoin,
    Wait,
    Abort,
}
#[derive(Debug)]
pub(super) struct Contained;
impl Contained {
    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(5)
    }
    fn command(mode: &str) -> Result<Command, std::io::Error> {
        let mut command = Command::new(std::env::current_exe()?);
        command.arg(mode).env_clear();
        Ok(command)
    }
    pub(super) fn child() -> Result<(), Box<dyn std::error::Error>> {
        std::io::stdout().write_all(b"R")?;
        std::io::stdout().flush()?;
        loop {
            std::thread::park();
        }
    }
    pub(super) fn helper(mode: HelperMode) -> Result<(), Box<dyn std::error::Error>> {
        let channel = Supervision::from_stdin()?.exchange_until(Self::deadline())?;
        if matches!(mode, HelperMode::RejectJoin) {
            return Ok(());
        }
        let channel = channel.join_domain_until(Self::deadline())?;
        let mut child = Self::command("contained-child")?
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()?;
        let mut ready = [0];
        child
            .stdout
            .take()
            .ok_or("child readiness pipe missing")?
            .read_exact(&mut ready)?;
        assert_eq!(ready, *b"R");
        UnixDatagram::unbound()?.send_to(b"ready", "/work/contained-ready.sock")?;
        if matches!(mode, HelperMode::Abort) {
            // Deliberately skip all Rust destructors after the child is ready.
            std::process::abort();
        }
        // The supervisor owns containment cancellation and descendant reaping.
        channel.wait_for_loss_until(Self::deadline())?;
        drop(child);
        Ok(())
    }
    fn clean(directory: &Path, parent: &Path) -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(fs::read_dir(directory)?.count(), 0);
        for entry in fs::read_dir(parent)? {
            assert!(!entry?.file_type()?.is_dir());
        }
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        Ok(())
    }
    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        let directory = Path::new("/work/contained-records");
        let parent = Path::new("/work/cgroups");
        fs::create_dir(directory)?;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
        let lease = RecoveryDirectory::open(directory)?;
        let mut reaper = ChildReaper::install()?;
        for reject_join in [false, true] {
            let domain = CommandDomain::create(&lease, parent)?;
            let command = if reject_join {
                Self::command("contained-reject")?
            } else {
                Command::new("/missing-owned-fixture-executable")
            };
            let failure = domain.launch(command, Self::deadline()).unwrap_err();
            let cancellation = match (*failure, reject_join) {
                (
                    HelperLaunchFailure::Launch {
                        source: LaunchFailure::Before(error),
                        domain,
                    },
                    false,
                ) => {
                    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
                    domain
                }
                (
                    HelperLaunchFailure::Join {
                        process, domain, ..
                    },
                    true,
                ) => {
                    assert!(
                        process
                            .wait_until(Self::deadline())
                            .map_err(|e| e.to_string())?
                            .status()
                            .success()
                    );
                    domain
                }
                (failure, _) => return Err(format!("unexpected launch failure: {failure}").into()),
            };
            cancellation
                .retry_until(Self::deadline())
                .map_err(|e| e.to_string())?;
            let drained = reaper
                .drain_until(Self::deadline())
                .map_err(|e| e.to_string())?;
            reaper = drained.into_reaper();
            Self::clean(directory, parent)?;
            println!(
                "CONTAINED_{}_FAILURE_CLEANUP_PASS",
                if reject_join { "JOIN" } else { "LAUNCH" }
            );
        }
        let ready = UnixDatagram::bind("/work/contained-ready.sock")?;
        ready.set_read_timeout(Some(Duration::from_secs(5)))?;
        for abrupt in [false, true] {
            let domain = CommandDomain::create(&lease, parent)?;
            let domain_path = parent.join(domain.name());
            let helper = domain
                .launch(
                    Self::command(if abrupt {
                        "contained-abort"
                    } else {
                        "contained-helper"
                    })?,
                    Self::deadline(),
                )
                .map_err(|e| e.to_string())?;
            let mut bytes = [0; 16];
            let count = ready.recv(&mut bytes)?;
            assert_eq!(&bytes[..count], b"ready");
            if abrupt {
                helper
                    .supervision()
                    .wait_for_peer_exit_until(Self::deadline())?;
                assert!(
                    fs::read_to_string(domain_path.join("cgroup.events"))?
                        .lines()
                        .any(|line| line == "populated 1")
                );
                assert_eq!(fs::read_dir(directory)?.count(), 0);
            }
            let (status, children) = helper
                .cancel_until(Self::deadline())
                .map_err(|e| e.to_string())?
                .reap_until(Self::deadline())
                .map_err(|e| e.to_string())?
                .with_reaper(reaper)
                .drain_until(Self::deadline())
                .map_err(|e| e.to_string())?
                .finish();
            if abrupt {
                assert_eq!(status.signal(), Some(6));
            }
            assert!(children.collected() >= 1);
            reaper = children.into_reaper();
            Self::clean(directory, parent)?;
            println!(
                "{}",
                if abrupt {
                    "CONTAINED_ABRUPT_HELPER_DEATH_DRAIN_PASS"
                } else {
                    "CONTAINED_PRE_ADMISSION_DESCENDANT_DRAIN_PASS"
                }
            );
        }
        drop(ready);
        fs::remove_file("/work/contained-ready.sock")?;
        drop(lease);
        fs::remove_dir(directory)?;
        Ok(())
    }
}
