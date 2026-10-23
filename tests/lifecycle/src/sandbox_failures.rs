// SPDX-License-Identifier: Apache-2.0
use conglobate::{
    execution::HelperProgram,
    formatter::{CapturedFormatter, Ext4Format},
    isolation::IdRange,
    path::ContextPath,
    process::ChildReaper,
    step::{CommandSandbox, SetupBudget},
    storage::{Created, Mapping, Runtime, Unmounted, Volume, Workspace},
};
use std::{fs, num::NonZeroU64, os::unix::fs::PermissionsExt, path::Path};

#[derive(Debug, Clone, Copy)]
pub(super) enum FailurePoint {
    Namespace,
    Mount,
    Launch,
    Join,
    AttachedExit,
    Gate,
}
#[derive(Debug)]
pub(super) struct SandboxFailures;
impl SandboxFailures {
    pub(super) fn attached_helper(
        cutoff: &std::ffi::OsStr,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use conglobate::{
            filesystem::MountPoint,
            isolation::{CommandDevices, PrivateMountNamespace},
            process::Supervision,
        };
        use std::os::unix::fs::MetadataExt;
        let cutoff = conglobate::step::SetupCutoff::from_argument(
            cutoff.to_str().ok_or("non-UTF8 cutoff")?,
        )?;
        let deadline = cutoff.deadline()?;
        let channel = Supervision::from_stdin()?
            .exchange_until(deadline)?
            .join_domain_until(deadline)?;
        eprintln!("attached fixture: joined");
        let input = channel.receive_volume_until(deadline)?;
        eprintln!("attached fixture: received volume");
        let scope = PrivateMountNamespace::enter()?;
        let (view, _setup, resolver) =
            input.prepare(&scope, CommandDevices::create(100000, 100000)?)?;
        assert!(resolver.is_none());
        eprintln!("attached fixture: prepared view");
        let view = view
            .attach(MountPoint::open(Path::new(
                "/work/runtime-parent/receiver-view",
            ))?)
            .map_err(|e| e.to_string())?;
        assert_eq!(view.root_metadata()?.uid(), 100000);
        fs::write("/work/receiver-stage", "attached")?;
        eprintln!("attached fixture: attached and recorded");
        // Trusted fixture process exits with live view owners and no destructors.
        std::process::exit(73);
    }

    pub(super) fn run(point: FailurePoint) -> Result<(), Box<dyn std::error::Error>> {
        let workspace = Workspace::create()?;
        let mut scratch = workspace.scratch()?;
        scratch.set_len(32 * 1024 * 1024)?;
        let sectors = NonZeroU64::new(32768).unwrap();
        let origin = Mapping::<Created>::create()?.zero(sectors)?;
        let snapshot = Mapping::<Created>::create()?
            .snapshot(
                &origin,
                scratch.attach().map_err(|e| e.to_string())?,
                sectors,
            )
            .map_err(|e| e.to_string())?;
        let snapshot = if matches!(
            point,
            FailurePoint::Launch
                | FailurePoint::Join
                | FailurePoint::AttachedExit
                | FailurePoint::Gate
        ) {
            let files = fs::read_to_string("/formatter-runtime-files")?
                .lines()
                .map(|line| ContextPath::try_from(Path::new(line)))
                .collect::<Result<Vec<_>, _>>()?;
            let formatter = CapturedFormatter::capture(
                Path::new("/"),
                ContextPath::try_from(Path::new("usr/bin/mke2fs"))?,
                files,
            )?
            .with_ext4_configuration()?
            .stage(&workspace)?;
            let (formatter, snapshot) = formatter
                .format(
                    snapshot,
                    Ext4Format::new("16MiB".parse()?, "rollback-probe".to_owned().try_into()?),
                    &HelperProgram::current()?,
                    SetupBudget::begin(),
                )
                .map_err(|e| e.to_string())?
                .wait()
                .map_err(|e| e.to_string())?
                .into_parts();
            formatter.close()?;
            snapshot
        } else {
            snapshot
        };
        let device = snapshot.id();
        let runtime_parent = Path::new("/work/runtime-parent");
        fs::create_dir(runtime_parent)?;
        fs::set_permissions(runtime_parent, fs::Permissions::from_mode(0o700))?;
        let runtime = Runtime::create(&workspace, runtime_parent).map_err(|e| e.to_string())?;
        let paths = runtime.paths()?;
        let mut volume = Volume::<Unmounted>::in_init_namespace(snapshot, runtime.recovery())
            .map_err(|e| e.to_string())?;
        if matches!(
            point,
            FailurePoint::Launch
                | FailurePoint::Join
                | FailurePoint::AttachedExit
                | FailurePoint::Gate
        ) {
            let mounted = volume
                .mount_ext4(paths.volumes())
                .map_err(|e| e.to_string())?;
            let entries = fs::read_dir(paths.volumes())?.collect::<Result<Vec<_>, _>>()?;
            assert_eq!(entries.len(), 1);
            for name in ["dev", "proc", "sys"] {
                fs::create_dir(entries[0].path().join(name))?;
            }
            volume = mounted.unmount().map_err(|e| e.to_string())?;
        }
        let sandbox = CommandSandbox::new(
            HelperProgram::current()?,
            paths.sandbox(),
            IdRange::new(100000, 65536)?,
            IdRange::new(100000, 65536)?,
        );
        let reaper = ChildReaper::install()?;
        let limit = Path::new("/proc/sys/user/max_user_namespaces");
        let original = fs::read_to_string(limit)?;
        if matches!(point, FailurePoint::Namespace) {
            // This sysctl exists only in the disposable guest. Restore before
            // assertions or rollback; no command can run in this fixture.
            fs::write(limit, "0")?;
        }
        let budget = SetupBudget::begin();
        let result = sandbox.prepare(volume, reaper, &budget);
        if matches!(point, FailurePoint::Namespace) {
            fs::write(limit, original)?;
        }
        let returned = match point {
            FailurePoint::Namespace | FailurePoint::Mount => {
                let failure = result.unwrap_err();
                let kind = failure.diagnostics().operation().kind();
                let errno = failure.diagnostics().operation().raw_os_error();
                assert_ne!(kind, std::io::ErrorKind::TimedOut);
                if matches!(point, FailurePoint::Mount) {
                    assert_eq!(errno, Some(22));
                }
                let (diagnostics, returned) =
                    failure.rollback().map_err(|e| e.to_string())?.into_parts();
                assert_eq!(diagnostics.operation().kind(), kind);
                assert_eq!(diagnostics.operation().raw_os_error(), errno);
                assert!(diagnostics.cleanup().is_empty());
                returned
            }
            FailurePoint::Launch
            | FailurePoint::Join
            | FailurePoint::AttachedExit
            | FailurePoint::Gate => {
                let prepared = result.map_err(|e| e.to_string())?;
                // Preparation has acquired the volume mount, namespace and domain.
                assert_eq!(fs::read_dir(paths.volumes())?.count(), 1);
                assert_eq!(fs::read_dir(paths.recovery())?.count(), 1);
                let mut command = if matches!(point, FailurePoint::Launch) {
                    std::process::Command::new("/nonexistent-fixture-helper")
                } else {
                    let mut command = std::process::Command::new(std::env::current_exe()?);
                    command.arg(if matches!(point, FailurePoint::Join) {
                        "contained-reject"
                    } else if matches!(point, FailurePoint::Gate) {
                        "sandbox-gate-receiver"
                    } else {
                        "sandbox-attached-helper"
                    });
                    command
                };
                command.env_clear();
                if matches!(point, FailurePoint::Gate) {
                    use std::{
                        io::{BufRead, BufReader},
                        os::unix::net::{UnixDatagram, UnixStream},
                        process::Stdio,
                        time::{Duration, Instant},
                    };
                    let (diagnostics, writer) = UnixStream::pair()?;
                    diagnostics.set_read_timeout(Some(Duration::from_secs(10)))?;
                    command.stderr(Stdio::from(std::os::fd::OwnedFd::from(writer)));
                    command.arg(budget.cutoff()?.to_argument());
                    fs::create_dir("/work/runtime-parent/receiver-view")?;
                    let step = prepared
                        .launch(command, budget.deadline(), paths.domains())
                        .map_err(|e| e.to_string())?;
                    step.send_volume_until(budget.deadline())?;
                    let command = budget
                        .await_prepared(step)
                        .map_err(|e| e.to_string())?
                        .start("10s".parse()?)
                        .map_err(|e| e.to_string())?;
                    let mut diagnostics = BufReader::new(diagnostics);
                    let mut line = String::new();
                    loop {
                        line.clear();
                        assert_ne!(diagnostics.read_line(&mut line)?, 0);
                        eprint!("{line}");
                        if line.starts_with("restricted command startup failed:") {
                            break;
                        }
                    }
                    // The diagnostic is emitted on entering local cleanup. The
                    // child cannot exit before this check and explicit release.
                    assert!(!command.supervision().peer_lost()?);
                    println!("RECEIVER_CONTROL_HELD_DURING_GATE_FAILURE_CLEANUP_PASS");
                    UnixDatagram::unbound()?
                        .send_to(b"exit", "/work/runtime-parent/gate-release.sock")?;
                    command
                        .supervision()
                        .wait_for_peer_exit_until(Instant::now() + Duration::from_secs(10))?;
                    let (outcome, completion) =
                        command.observe().teardown(paths.domains()).into_parts();
                    assert!(!outcome.success());
                    let finalized = completion
                        .map_err(|e| e.to_string())?
                        .unmount()
                        .map_err(|e| e.to_string())?;
                    let (status, _, returned) = finalized.into_parts();
                    assert!(status.success());
                    assert_eq!(
                        fs::read_to_string("/work/receiver-gate-cleaned")?,
                        "cleaned"
                    );
                    fs::remove_file("/work/receiver-gate-cleaned")?;
                    fs::remove_dir("/work/runtime-parent/receiver-view")?;
                    returned
                } else if matches!(point, FailurePoint::AttachedExit) {
                    fs::create_dir("/work/runtime-parent/receiver-view")?;
                    command.arg(budget.cutoff()?.to_argument());
                    let step = prepared
                        .launch(command, budget.deadline(), paths.domains())
                        .map_err(|e| e.to_string())?;
                    let failure = step
                        .execute(budget, "5s".parse()?, paths.domains())
                        .unwrap_err();
                    eprintln!("attached fixture outcome: {failure}");
                    assert!(matches!(
                        failure.outcome(),
                        conglobate::step::ExecutionOutcome::SetupFailed(_)
                    ));
                    assert!(failure.cleanup_error().is_none());
                    let (_, returned) =
                        failure.into_rejected_storage().map_err(|e| e.to_string())?;
                    assert_eq!(fs::read_to_string("/work/receiver-stage")?, "attached");
                    assert_eq!(
                        fs::read_dir("/work/runtime-parent/receiver-view")?.count(),
                        0
                    );
                    fs::remove_dir("/work/runtime-parent/receiver-view")?;
                    fs::remove_file("/work/receiver-stage")?;
                    returned
                } else {
                    let failure = prepared
                        .launch(command, budget.deadline(), paths.domains())
                        .unwrap_err();
                    let kind = failure.diagnostics().operation().kind();
                    if matches!(point, FailurePoint::Launch) {
                        assert_eq!(kind, std::io::ErrorKind::NotFound);
                    }
                    assert_ne!(kind, std::io::ErrorKind::TimedOut);
                    let (diagnostics, returned) =
                        failure.rollback().map_err(|e| e.to_string())?.into_parts();
                    assert_eq!(diagnostics.operation().kind(), kind);
                    assert!(diagnostics.cleanup().is_empty());
                    returned
                }
            }
        };
        assert_eq!(fs::read_dir(paths.recovery())?.count(), 0);
        assert_eq!(fs::read_dir(paths.volumes())?.count(), 0);
        for entry in fs::read_dir(paths.domains())? {
            assert!(!entry?.file_type()?.is_dir());
        }
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        let snapshot = returned.into_snapshot();
        snapshot.usage()?;
        snapshot
            .close()
            .map_err(|e| e.to_string())?
            .detach()
            .map_err(|e| e.to_string())?;
        assert!(
            !Path::new(&format!(
                "/sys/dev/block/{}:{}",
                device.major(),
                device.minor()
            ))
            .exists()
        );
        origin.close()?;
        drop(sandbox);
        drop(paths);
        runtime.close().map_err(|e| e.to_string())?;
        assert_eq!(fs::read_dir(runtime_parent)?.count(), 0);
        fs::remove_dir(runtime_parent)?;
        println!(
            "{}",
            match point {
                FailurePoint::Namespace => "SANDBOX_NAMESPACE_FAILURE_ROLLBACK_PASS",
                FailurePoint::Mount => "SANDBOX_MOUNT_FAILURE_ROLLBACK_PASS",
                FailurePoint::Launch => "SANDBOX_LAUNCH_FAILURE_ROLLBACK_PASS",
                FailurePoint::Join => "SANDBOX_JOIN_FAILURE_ROLLBACK_PASS",
                FailurePoint::AttachedExit => "SANDBOX_ATTACHED_EXIT_ROLLBACK_PASS",
                FailurePoint::Gate => "SANDBOX_GATE_FAILURE_ROLLBACK_PASS",
            }
        );
        Ok(())
    }
}
