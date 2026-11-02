// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
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
    StartGate,
    UnmappedOwner,
    UnmappedAcl,
    UnmappedDefaultGroup,
    BusyExecution,
    HeldCleanup,
    MissingCleanup,
    CleanupTimeout,
    DelayedTeardown,
    Gate,
    Writeback,
    BlockedIo,
    BlockedDescendant,
    SupervisorDeath,
}
#[derive(Debug, Clone, Copy)]
pub(super) enum HelperCompletion {
    None,
    Normal,
    Held,
    Missing,
    Never,
}
#[derive(Debug)]
pub(super) struct SandboxFailures;
impl SandboxFailures {
    fn io_error(error: &anyhow::Error) -> &std::io::Error {
        error
            .chain()
            .find_map(|cause| cause.downcast_ref())
            .expect("underlying I/O error")
    }

    fn assert_owned_domain(parent: &Path) -> std::io::Result<()> {
        let mut domains = Vec::new();
        for entry in fs::read_dir(parent)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                domains.push(entry.path());
            }
        }
        assert_eq!(domains.len(), 1);
        assert_eq!(
            fs::read_to_string(domains[0].join("cgroup.type"))?.trim(),
            "domain"
        );
        Ok(())
    }

    pub(super) fn attached_helper(
        cutoff: &std::ffi::OsStr,
        completion: HelperCompletion,
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
        if !matches!(completion, HelperCompletion::None) {
            use std::os::unix::process::ExitStatusExt;
            let reporter = channel
                .receive_gate_until(deadline)?
                .ready_for_reporting(deadline)?;
            let release = if matches!(completion, HelperCompletion::Held) {
                Some(std::os::unix::net::UnixDatagram::bind(
                    "/work/runtime-parent/cleanup-release.sock",
                )?)
            } else {
                None
            };
            reporter.report(std::process::ExitStatus::from_raw(0))?;
            if matches!(completion, HelperCompletion::Never) {
                loop {
                    std::thread::park();
                }
            }
            if let Some(release) = release {
                release.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
                let mut packet = [0; 2];
                assert_eq!(release.recv(&mut packet)?, 2);
                assert_eq!(&packet, b"go");
                fs::remove_file("/work/runtime-parent/cleanup-release.sock")?;
            }
            view.close().map_err(|e| e.to_string())?;
            drop(_setup);
            scope.restore().map_err(|e| e.to_string())?;
            if !matches!(completion, HelperCompletion::Missing) {
                channel.report_cleanup_until(std::process::ExitStatus::from_raw(0), deadline)?;
            }
            return Ok(());
        }
        fs::write("/work/receiver-stage", "attached")?;
        eprintln!("attached fixture: attached and recorded");
        // Trusted fixture process exits with live view owners and no destructors.
        std::process::exit(73);
    }

    pub(super) fn run(point: FailurePoint) -> Result<(), Box<dyn std::error::Error>> {
        let workspace = Workspace::create()?;
        let mut scratch = workspace.scratch()?;
        scratch.set_len(if matches!(point, FailurePoint::Writeback) {
            8 * 1024 * 1024
        } else {
            32 * 1024 * 1024
        })?;
        // Root capacity plus GPT and alignment space.
        let sectors = NonZeroU64::new(18 * 1024 * 1024 / 512).unwrap();
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
                | FailurePoint::StartGate
                | FailurePoint::UnmappedOwner
                | FailurePoint::UnmappedAcl
                | FailurePoint::UnmappedDefaultGroup
                | FailurePoint::BusyExecution
                | FailurePoint::HeldCleanup
                | FailurePoint::MissingCleanup
                | FailurePoint::DelayedTeardown
                | FailurePoint::CleanupTimeout
                | FailurePoint::Gate
                | FailurePoint::Writeback
                | FailurePoint::BlockedDescendant
                | FailurePoint::BlockedIo
                | FailurePoint::SupervisorDeath
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
                .into_parts()?;
            formatter.close()?;
            snapshot
        } else {
            // Namespace and mount-failure cases still need valid disk metadata.
            // Leave the root partition unformatted to exercise mount rejection.
            conglobate::storage::PartitionedSnapshot::create(
                snapshot,
                &Ext4Format::new("16MiB".parse()?, Default::default()).layout()?,
            )?
            .into_snapshot()?
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
                | FailurePoint::StartGate
                | FailurePoint::UnmappedOwner
                | FailurePoint::UnmappedAcl
                | FailurePoint::UnmappedDefaultGroup
                | FailurePoint::BusyExecution
                | FailurePoint::HeldCleanup
                | FailurePoint::MissingCleanup
                | FailurePoint::DelayedTeardown
                | FailurePoint::CleanupTimeout
                | FailurePoint::Gate
                | FailurePoint::Writeback
                | FailurePoint::BlockedDescendant
                | FailurePoint::BlockedIo
                | FailurePoint::SupervisorDeath
        ) {
            let mounted = volume
                .mount_ext4(paths.volumes())
                .map_err(|e| e.to_string())?;
            let entries = fs::read_dir(paths.volumes())?.collect::<Result<Vec<_>, _>>()?;
            assert_eq!(entries.len(), 1);
            for name in ["dev", "proc", "sys"] {
                fs::create_dir(entries[0].path().join(name))?;
            }
            fs::write(entries[0].path().join("blocked-writer"), [])?;
            if matches!(point, FailurePoint::UnmappedOwner) {
                for (name, id) in [
                    ("last-mapped-owner", 65535),
                    ("first-unmapped-owner", 65536),
                ] {
                    let file = entries[0].path().join(name);
                    fs::write(&file, b"ordinary inherited ownership fixture")?;
                    std::os::unix::fs::chown(file, Some(id), Some(id))?;
                }
            }
            if matches!(
                point,
                FailurePoint::UnmappedAcl | FailurePoint::UnmappedDefaultGroup
            ) {
                let path = entries[0].path().join("named-acl-owner");
                let (file, attribute, entries) =
                    if matches!(point, FailurePoint::UnmappedDefaultGroup) {
                        fs::create_dir(&path)?;
                        (
                            fs::File::open(path)?,
                            "system.posix_acl_default",
                            [
                                (1u16, 7u16, u32::MAX),
                                (4, 5, u32::MAX),
                                (8, 4, 65536),
                                (16, 7, u32::MAX),
                                (32, 5, u32::MAX),
                            ],
                        )
                    } else {
                        (
                            fs::File::create(path)?,
                            "system.posix_acl_access",
                            [
                                (1u16, 7u16, u32::MAX),
                                (2, 4, 65536),
                                (4, 5, u32::MAX),
                                (16, 7, u32::MAX),
                                (32, 5, u32::MAX),
                            ],
                        )
                    };
                let mut acl = 2u32.to_le_bytes().to_vec();
                for (tag, permissions, id) in entries {
                    acl.extend_from_slice(&tag.to_le_bytes());
                    acl.extend_from_slice(&permissions.to_le_bytes());
                    acl.extend_from_slice(&id.to_le_bytes());
                }
                rustix::fs::fsetxattr(&file, attribute, &acl, rustix::fs::XattrFlags::empty())?;
            }
            if matches!(point, FailurePoint::BlockedDescendant) {
                crate::fixture::blocked_descendant::BlockedDescendant::stage(&entries[0].path())?;
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
            FailurePoint::Namespace
            | FailurePoint::Mount
            | FailurePoint::UnmappedOwner
            | FailurePoint::UnmappedAcl
            | FailurePoint::UnmappedDefaultGroup => {
                let failure = result.unwrap_err();
                let kind = Self::io_error(&failure).kind();
                let errno = Self::io_error(&failure).raw_os_error();
                assert_ne!(kind, std::io::ErrorKind::TimedOut);
                if matches!(
                    point,
                    FailurePoint::UnmappedOwner
                        | FailurePoint::UnmappedAcl
                        | FailurePoint::UnmappedDefaultGroup
                ) {
                    assert_eq!(kind, std::io::ErrorKind::InvalidData);
                    assert!(Self::io_error(&failure).to_string().contains(
                        if matches!(point, FailurePoint::UnmappedDefaultGroup) {
                            "unmapped image GID 65536"
                        } else {
                            "unmapped image UID 65536"
                        }
                    ));
                    println!("UNMAPPED_IMAGE_ID_REJECTED_BEFORE_RECEIVER");
                }
                if matches!(point, FailurePoint::Mount) {
                    assert_eq!(errno, Some(22));
                }
                None
            }
            FailurePoint::Launch
            | FailurePoint::Join
            | FailurePoint::AttachedExit
            | FailurePoint::StartGate
            | FailurePoint::BusyExecution
            | FailurePoint::HeldCleanup
            | FailurePoint::MissingCleanup
            | FailurePoint::DelayedTeardown
            | FailurePoint::CleanupTimeout
            | FailurePoint::Gate
            | FailurePoint::Writeback
            | FailurePoint::BlockedDescendant
            | FailurePoint::BlockedIo
            | FailurePoint::SupervisorDeath => {
                let prepared = result.map_err(|e| e.to_string())?;
                // Preparation has acquired the volume mount, namespace and domain.
                assert_eq!(fs::read_dir(paths.volumes())?.count(), 1);
                Self::assert_owned_domain(paths.domains())?;
                assert_eq!(fs::read_dir(paths.recovery())?.count(), 0);
                if matches!(point, FailurePoint::BlockedDescendant) {
                    use crate::fixture::blocked_descendant::BlockedDescendant;
                    use std::{
                        io::{BufRead, BufReader},
                        os::fd::OwnedFd,
                        os::unix::net::UnixStream,
                        process::Stdio,
                        time::{Duration, Instant},
                    };
                    let receiver_parent = runtime_parent.join("receiver-view");
                    fs::create_dir(&receiver_parent)?;
                    let mut receiver = HelperProgram::current()?.receiver(
                        BlockedDescendant::SCRIPT.to_owned(),
                        budget.cutoff()?,
                        100000,
                        100000,
                        &receiver_parent,
                    )?;
                    let (reader, writer) = UnixStream::pair()?;
                    reader.set_read_timeout(Some(Duration::from_secs(10)))?;
                    receiver.stderr(Stdio::from(OwnedFd::from(writer)));
                    let step = prepared
                        .launch(receiver, budget.deadline())
                        .map_err(|e| e.to_string())?;
                    step.send_volume_until(budget.deadline())?;
                    let admitted = match budget.await_prepared(step) {
                        Ok(admitted) => admitted,
                        Err(error) => {
                            use std::io::Read;
                            let mut diagnostics = String::new();
                            let _ = (&reader).read_to_string(&mut diagnostics);
                            eprint!("{diagnostics}");
                            return Err(error.to_string().into());
                        }
                    };
                    let running = admitted.start("10s".parse()?).map_err(|e| e.to_string())?;
                    let mut reader = BufReader::new(reader);
                    let mut line = String::new();
                    loop {
                        line.clear();
                        assert_ne!(reader.read_line(&mut line)?, 0);
                        eprint!("{line}");
                        if line.starts_with("BLOCKED_PLAN_READY ") {
                            break;
                        }
                    }
                    let ids = line
                        .split_whitespace()
                        .skip(1)
                        .map(str::parse::<u32>)
                        .collect::<Result<Vec<_>, _>>()?;
                    assert_eq!(ids.len(), 2);
                    let shell = BlockedDescendant::host_pid(ids[0])?;
                    let child = BlockedDescendant::host_pid(ids[1])?;
                    assert_eq!(
                        fs::read_link(format!("/proc/{shell}/ns/pid"))?,
                        fs::read_link(format!("/proc/{child}/ns/pid"))?
                    );
                    assert_ne!(
                        fs::read_link(format!("/proc/{shell}/ns/pid"))?,
                        fs::read_link("/proc/self/ns/pid")?
                    );
                    BlockedDescendant::await_state(child, 'T')?;
                    let sysfs = format!("/sys/dev/block/{}:{}", device.major(), device.minor());
                    let name = fs::read_to_string(Path::new(&sysfs).join("dm/name"))?;
                    assert!(
                        std::process::Command::new("/usr/bin/dmsetup")
                            .args(["suspend", "--noflush", "--nolockfs", name.trim()])
                            .status()?
                            .success()
                    );
                    BlockedDescendant::signal(child, "CONT")?;
                    BlockedDescendant::await_state(child, 'D')?;
                    println!("PLAN_DESCENDANT_UNINTERRUPTIBLE_PASS");
                    BlockedDescendant::signal(shell, "USR1")?;
                    let observed = running.observe();
                    assert!(observed.outcome().success(), "{}", observed.outcome());
                    println!("SHELL_SUCCESS_BEFORE_BLOCKED_DESCENDANT_CLEANUP_PASS");
                    let started = Instant::now();
                    let (outcome, cleanup) = observed.teardown().into_parts();
                    assert!(outcome.success());
                    let failure = cleanup.unwrap_err();
                    assert_eq!(failure.kind(), std::io::ErrorKind::TimedOut);
                    BlockedDescendant::await_state(child, 'D')?;
                    assert!(started.elapsed() >= Duration::from_secs(30));
                    assert!(started.elapsed() < Duration::from_secs(40));
                    assert_eq!(fs::read_dir(paths.volumes())?.count(), 1);
                    Self::assert_owned_domain(paths.domains())?;
                    assert_eq!(fs::read_dir(paths.recovery())?.count(), 0);
                    assert!(Path::new(&sysfs).exists());
                    let retry_started = Instant::now();
                    assert!(retry_started.elapsed() < Duration::from_secs(1));
                    assert!(outcome.success());
                    assert_eq!(fs::read_dir(paths.volumes())?.count(), 1);
                    Self::assert_owned_domain(paths.domains())?;
                    assert_eq!(fs::read_dir(paths.recovery())?.count(), 0);
                    println!("BLOCKED_DESCENDANT_TEARDOWN_RETAINS_SUCCESS_AND_STORAGE_PASS");
                    // Retain failed owners until independent guest init disposes of the VM.
                    std::process::exit(0);
                }
                let mut command = if matches!(point, FailurePoint::Launch) {
                    std::process::Command::new("/nonexistent-fixture-helper")
                } else {
                    let mut command = std::process::Command::new(std::env::current_exe()?);
                    command.arg(if matches!(point, FailurePoint::Join) {
                        "contained-reject"
                    } else if matches!(
                        point,
                        FailurePoint::BlockedIo | FailurePoint::SupervisorDeath
                    ) {
                        "sandbox-blocked-helper"
                    } else if matches!(point, FailurePoint::StartGate) {
                        "sandbox-start-reject-receiver"
                    } else if matches!(point, FailurePoint::Gate | FailurePoint::Writeback) {
                        "sandbox-gate-receiver"
                    } else if matches!(point, FailurePoint::HeldCleanup) {
                        "sandbox-held-helper"
                    } else if matches!(
                        point,
                        FailurePoint::CleanupTimeout | FailurePoint::DelayedTeardown
                    ) {
                        "sandbox-cleanup-timeout-helper"
                    } else if matches!(point, FailurePoint::MissingCleanup) {
                        "sandbox-missing-cleanup-helper"
                    } else if matches!(point, FailurePoint::BusyExecution) {
                        "sandbox-success-helper"
                    } else {
                        "sandbox-attached-helper"
                    });
                    command
                };
                command.env_clear();
                if matches!(
                    point,
                    FailurePoint::BlockedIo | FailurePoint::SupervisorDeath
                ) {
                    use std::{
                        os::unix::net::UnixDatagram,
                        time::{Duration, Instant},
                    };
                    fs::create_dir("/work/runtime-parent/receiver-view")?;
                    let ready = UnixDatagram::bind("/work/runtime-parent/blocked-ready.sock")?;
                    ready.set_read_timeout(Some(Duration::from_secs(10)))?;
                    command.arg(budget.cutoff()?.to_argument());
                    let step = prepared
                        .launch(command, budget.deadline())
                        .map_err(|e| e.to_string())?;
                    step.send_volume_until(budget.deadline())?;
                    let command = budget
                        .await_prepared(step)
                        .map_err(|e| e.to_string())?
                        .start("5s".parse()?)
                        .map_err(|e| e.to_string())?;
                    let mut packet = [0; 4];
                    assert_eq!(ready.recv(&mut packet)?, 4);
                    let helper = u32::from_ne_bytes(packet);
                    let sysfs = format!("/sys/dev/block/{}:{}", device.major(), device.minor());
                    let name = fs::read_to_string(Path::new(&sysfs).join("dm/name"))?;
                    assert!(
                        std::process::Command::new("/usr/bin/dmsetup")
                            .args(["suspend", "--noflush", "--nolockfs", name.trim()])
                            .status()?
                            .success()
                    );
                    ready.send_to(b"go", "/work/runtime-parent/blocked-release.sock")?;
                    let blocked_deadline = Instant::now() + Duration::from_secs(3);
                    loop {
                        let status = fs::read_to_string(format!("/proc/{helper}/status"))?;
                        if status.lines().any(|line| line.starts_with("State:\tD")) {
                            break;
                        }
                        assert!(
                            Instant::now() < blocked_deadline,
                            "fixture did not enter uninterruptible I/O"
                        );
                        std::thread::yield_now();
                    }
                    println!("FILESYSTEM_WRITER_UNINTERRUPTIBLE_PASS");
                    if matches!(point, FailurePoint::SupervisorDeath) {
                        println!("SUPERVISOR_ABRUPT_DEATH_INJECTED");
                        std::process::abort();
                    }
                    let observed = command.observe();
                    assert!(
                        matches!(observed.outcome(), conglobate::step::CommandOutcome::ObservationFailed(error)
                        if error.kind() == std::io::ErrorKind::TimedOut)
                    );
                    let started = Instant::now();
                    let (outcome, cleanup) = observed.teardown().into_parts();
                    assert!(!outcome.success());
                    let failure = cleanup.unwrap_err();
                    assert!(started.elapsed() >= Duration::from_secs(30));
                    assert!(started.elapsed() < Duration::from_secs(40));
                    eprintln!("blocked I/O cleanup: {failure}");
                    assert!(Path::new(&sysfs).exists());
                    assert_eq!(fs::read_dir(paths.volumes())?.count(), 1);
                    // Process teardown already retired the domain record;
                    // storage remains owned independently by finalization.
                    assert_eq!(fs::read_dir(paths.recovery())?.count(), 0);
                    assert!(
                        fs::read_to_string(format!("/proc/{helper}/status"))?
                            .lines()
                            .any(|line| line.starts_with("State:\tD"))
                    );
                    let retry_started = Instant::now();
                    assert!(retry_started.elapsed() < Duration::from_secs(1));
                    assert!(failure.to_string().contains("deadline expired"));
                    println!("BLOCKED_IO_FIXED_DEADLINE_RETAINS_STORAGE_PASS");
                    // Terminal case: no clean finalization, resource reuse or output
                    // is claimed. The independent guest init powers off this VM.
                    std::process::exit(0);
                } else if matches!(point, FailurePoint::Gate | FailurePoint::Writeback) {
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
                        .launch(command, budget.deadline())
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
                        if line.starts_with("restricted command completion held:") {
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
                    let completion = command
                        .observe()
                        .teardown()
                        .transpose()
                        .map_err(|failure| failure.cleanup().to_string())?;
                    assert!(matches!(
                        completion.outcome(),
                        conglobate::step::CommandOutcome::ObservationFailed(_)
                    ));
                    let primary = completion.outcome().to_string();
                    let entries = fs::read_dir(paths.volumes())?.collect::<Result<Vec<_>, _>>()?;
                    assert_eq!(entries.len(), 1);
                    if matches!(point, FailurePoint::Writeback) {
                        super::writeback::Writeback::reject(
                            completion,
                            &entries[0].path(),
                            Path::new(&format!(
                                "/sys/dev/block/{}:{}",
                                device.major(),
                                device.minor()
                            )),
                        )?;
                        unreachable!("terminal invalid-storage fixture");
                    }
                    // An ordinary open directory makes this owned mount busy.
                    // Keep the failure owner until the holder is explicitly closed.
                    let holder = fs::File::open(entries[0].path())?;
                    let failure = completion.unmount().unwrap_err();
                    assert_eq!(failure.outcome().to_string(), primary);
                    assert_eq!(
                        Self::io_error(failure.cleanup()).kind(),
                        std::io::ErrorKind::ResourceBusy
                    );
                    assert_eq!(fs::read_dir(paths.volumes())?.count(), 1);
                    // Process teardown already retired the domain record;
                    // storage remains owned independently by finalization.
                    assert_eq!(fs::read_dir(paths.recovery())?.count(), 0);
                    assert!(
                        Path::new(&format!(
                            "/sys/dev/block/{}:{}",
                            device.major(),
                            device.minor()
                        ))
                        .exists()
                    );
                    drop(holder);
                    assert_eq!(fs::read_dir(paths.volumes())?.count(), 1);
                    println!("PRIMARY_OUTCOME_THROUGH_BUSY_FINALIZATION_TERMINAL_PASS");
                    std::process::exit(0);
                } else if matches!(
                    point,
                    FailurePoint::HeldCleanup
                        | FailurePoint::MissingCleanup
                        | FailurePoint::DelayedTeardown
                        | FailurePoint::CleanupTimeout
                ) {
                    use std::{os::unix::net::UnixDatagram, time::Instant};
                    fs::create_dir("/work/runtime-parent/receiver-view")?;
                    command.arg(budget.cutoff()?.to_argument());
                    let step = prepared
                        .launch(command, budget.deadline())
                        .map_err(|e| e.to_string())?;
                    if matches!(point, FailurePoint::DelayedTeardown) {
                        step.send_volume_until(budget.deadline())?;
                        let observed = budget
                            .await_prepared(step)
                            .map_err(|e| e.to_string())?
                            .start("1s".parse()?)
                            .map_err(|e| e.to_string())?
                            .observe();
                        assert!(observed.outcome().success());
                        let deadline = observed.cleanup().teardown_deadline();
                        // Expiration is the test target, not a readiness signal.
                        while let Some(remaining) = deadline.checked_duration_since(Instant::now())
                        {
                            let timeout = rustix::event::Timespec::try_from(remaining)?;
                            match rustix::event::poll(&mut [], Some(&timeout)) {
                                Ok(_) | Err(rustix::io::Errno::INTR) => {}
                                Err(error) => return Err(error.into()),
                            }
                        }
                        let started = Instant::now();
                        let (outcome, cleanup) = observed.teardown().into_parts();
                        assert!(outcome.success());
                        let failure = cleanup.unwrap_err();
                        assert!(started.elapsed() < std::time::Duration::from_secs(1));
                        assert_eq!(failure.kind(), std::io::ErrorKind::TimedOut);
                        assert!(started.elapsed() < std::time::Duration::from_secs(1));
                        assert_eq!(fs::read_dir(paths.volumes())?.count(), 1);
                        Self::assert_owned_domain(paths.domains())?;
                        assert_eq!(fs::read_dir(paths.recovery())?.count(), 0);
                        println!("DELAYED_TEARDOWN_CANNOT_RENEW_OBSERVATION_CUTOFF_PASS");
                        std::process::exit(0);
                    }
                    if matches!(point, FailurePoint::CleanupTimeout) {
                        use std::time::Duration;
                        let started = Instant::now();
                        let failure = step.execute(budget, "1s".parse()?).unwrap_err();
                        assert!(
                            matches!(failure.downcast_ref::<conglobate::step::execution::ExecutionOutcome>(), Some(conglobate::step::execution::ExecutionOutcome::Command(conglobate::step::CommandOutcome::Exited(status))) if status.success())
                        );
                        assert_eq!(
                            Self::io_error(&failure).kind(),
                            std::io::ErrorKind::TimedOut
                        );
                        assert!(started.elapsed() >= Duration::from_secs(30));
                        assert!(started.elapsed() < Duration::from_secs(40));
                        assert_eq!(fs::read_dir(paths.volumes())?.count(), 1);
                        Self::assert_owned_domain(paths.domains())?;
                        assert_eq!(fs::read_dir(paths.recovery())?.count(), 0);
                        assert!(
                            Path::new(&format!(
                                "/sys/dev/block/{}:{}",
                                device.major(),
                                device.minor()
                            ))
                            .exists()
                        );
                        assert!(
                            !fs::read_to_string("/proc/thread-self/children")?
                                .trim()
                                .is_empty()
                        );
                        assert_eq!(fs::read_dir(paths.volumes())?.count(), 1);
                        Self::assert_owned_domain(paths.domains())?;
                        assert_eq!(fs::read_dir(paths.recovery())?.count(), 0);
                        println!(
                            "CLEANUP_DEADLINE_PRESERVES_SHELL_SUCCESS_AND_RETAINS_OWNERS_PASS"
                        );
                        // Deliberately abandon retained live owners; independent
                        // guest init disposes of this VM. No clean teardown claim.
                        std::process::exit(0);
                    }
                    let returned = if matches!(point, FailurePoint::HeldCleanup) {
                        step.send_volume_until(budget.deadline())?;
                        let running = budget
                            .await_prepared(step)
                            .map_err(|e| e.to_string())?
                            .start("1s".parse()?)
                            .map_err(|e| e.to_string())?;
                        let execution_deadline = running.deadline();
                        let observed = running.observe();
                        assert!(observed.outcome().success());
                        assert!(Path::new("/work/runtime-parent/cleanup-release.sock").exists());
                        assert_eq!(fs::read_dir(paths.volumes())?.count(), 1);
                        Self::assert_owned_domain(paths.domains())?;
                        assert_eq!(fs::read_dir(paths.recovery())?.count(), 0);
                        // Deliberately expire the execution timer after observing
                        // status. Cleanup is held by an explicit datagram gate;
                        // this timer wait is not a readiness synchronization.
                        while let Some(remaining) =
                            execution_deadline.checked_duration_since(Instant::now())
                        {
                            let timeout = rustix::event::Timespec::try_from(remaining)?;
                            match rustix::event::poll(&mut [], Some(&timeout)) {
                                Ok(_) | Err(rustix::io::Errno::INTR) => {}
                                Err(error) => return Err(error.into()),
                            }
                        }
                        UnixDatagram::unbound()?
                            .send_to(b"go", "/work/runtime-parent/cleanup-release.sock")?;
                        let (outcome, cleanup) = observed.teardown().into_parts();
                        assert!(outcome.success());
                        let finalized = cleanup
                            .map_err(|e| e.to_string())?
                            .unmount()
                            .map_err(|e| e.to_string())?;
                        let (_, _, returned) = finalized.into_parts();
                        println!("SHELL_STATUS_BEFORE_HELD_CLEANUP_PASS");
                        returned
                    } else {
                        let failure = step.execute(budget, "5s".parse()?).unwrap_err();
                        assert!(
                            matches!(failure.downcast_ref::<conglobate::step::execution::ExecutionOutcome>(), Some(conglobate::step::execution::ExecutionOutcome::Command(conglobate::step::CommandOutcome::Exited(status))) if status.success())
                        );
                        assert!(
                            format!("{failure:#}").contains("cleanup")
                                || failure.chain().count() > 1
                        );
                        assert_eq!(fs::read_dir(paths.volumes())?.count(), 1);
                        println!("MISSING_CLEANUP_RECEIPT_REJECTS_SHELL_SUCCESS_PASS");
                        std::process::exit(0);
                    };
                    fs::remove_dir("/work/runtime-parent/receiver-view")?;
                    Some(returned)
                } else if matches!(point, FailurePoint::BusyExecution) {
                    fs::create_dir("/work/runtime-parent/receiver-view")?;
                    command.arg(budget.cutoff()?.to_argument());
                    let entry = fs::read_dir(paths.volumes())?
                        .next()
                        .ok_or("missing volume")??;
                    let holder = fs::File::open(entry.path())?;
                    let step = prepared
                        .launch(command, budget.deadline())
                        .map_err(|e| e.to_string())?;
                    let failure = step.execute(budget, "5s".parse()?).unwrap_err();
                    assert!(
                        matches!(failure.downcast_ref::<conglobate::step::execution::ExecutionOutcome>(),
                        Some(conglobate::step::execution::ExecutionOutcome::Command(conglobate::step::CommandOutcome::Exited(status))) if status.success())
                    );
                    assert_eq!(
                        Self::io_error(&failure).kind(),
                        std::io::ErrorKind::ResourceBusy
                    );
                    drop(holder);
                    assert_eq!(fs::read_dir(paths.volumes())?.count(), 1);
                    println!("SUCCESSFUL_COMMAND_CLEANUP_FAILURE_STAYS_REJECTED_PASS");
                    std::process::exit(0);
                } else if matches!(point, FailurePoint::AttachedExit | FailurePoint::StartGate) {
                    fs::create_dir("/work/runtime-parent/receiver-view")?;
                    command.arg(budget.cutoff()?.to_argument());
                    let step = prepared
                        .launch(command, budget.deadline())
                        .map_err(|e| e.to_string())?;
                    let failure = step.execute(budget, "5s".parse()?).unwrap_err();
                    eprintln!("attached fixture outcome: {failure}");
                    assert!(matches!(
                        failure.downcast_ref::<conglobate::step::execution::ExecutionOutcome>(),
                        Some(conglobate::step::execution::ExecutionOutcome::SetupFailed(
                            _
                        ))
                    ));
                    assert!(format!("{failure:#}").contains("cleanup: complete"));
                    if matches!(point, FailurePoint::AttachedExit) {
                        assert_eq!(fs::read_to_string("/work/receiver-stage")?, "attached");
                        fs::remove_file("/work/receiver-stage")?;
                    } else {
                        assert_eq!(
                            Self::io_error(&failure).kind(),
                            std::io::ErrorKind::BrokenPipe
                        );
                    }
                    assert_eq!(
                        fs::read_dir("/work/runtime-parent/receiver-view")?.count(),
                        0
                    );
                    fs::remove_dir("/work/runtime-parent/receiver-view")?;
                    assert_eq!(fs::read_dir(paths.volumes())?.count(), 0);
                    assert!(
                        fs::read_to_string("/proc/thread-self/children")?
                            .trim()
                            .is_empty()
                    );
                    for entry in fs::read_dir(paths.domains())? {
                        assert!(!entry?.file_type()?.is_dir());
                    }
                    println!(
                        "{}",
                        if matches!(point, FailurePoint::StartGate) {
                            "SANDBOX_START_REJECT_ROLLBACK_PASS"
                        } else {
                            "SANDBOX_ATTACHED_EXIT_ROLLBACK_PASS"
                        }
                    );
                    std::process::exit(0);
                } else {
                    let failure = prepared.launch(command, budget.deadline()).unwrap_err();
                    let kind = Self::io_error(&failure).kind();
                    if matches!(point, FailurePoint::Launch) {
                        assert_eq!(kind, std::io::ErrorKind::NotFound);
                    }
                    assert_ne!(kind, std::io::ErrorKind::TimedOut);
                    None
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
        if let Some(returned) = returned {
            let snapshot = returned.into_snapshot()?;
            snapshot.usage()?;
            snapshot
                .close()
                .map_err(|e| e.to_string())?
                .detach()
                .map_err(|e| e.to_string())?;
        }
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
                FailurePoint::UnmappedOwner => "SANDBOX_UNMAPPED_OWNER_ROLLBACK_PASS",
                FailurePoint::UnmappedAcl => "SANDBOX_UNMAPPED_ACL_ROLLBACK_PASS",
                FailurePoint::UnmappedDefaultGroup =>
                    "SANDBOX_UNMAPPED_DEFAULT_GROUP_ROLLBACK_PASS",
                FailurePoint::BusyExecution => "SANDBOX_BUSY_EXECUTION_ROLLBACK_PASS",
                FailurePoint::HeldCleanup => "SANDBOX_HELD_CLEANUP_ROLLBACK_PASS",
                FailurePoint::MissingCleanup => "SANDBOX_MISSING_CLEANUP_ROLLBACK_PASS",
                FailurePoint::StartGate => "SANDBOX_START_REJECT_ROLLBACK_PASS",
                FailurePoint::Gate => "SANDBOX_GATE_FAILURE_ROLLBACK_PASS",
                FailurePoint::CleanupTimeout
                | FailurePoint::DelayedTeardown
                | FailurePoint::BlockedDescendant
                | FailurePoint::BlockedIo
                | FailurePoint::SupervisorDeath
                | FailurePoint::Writeback => unreachable!("terminal VM-disposal case"),
            }
        );
        Ok(())
    }
}
