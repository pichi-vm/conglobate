// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]
mod readiness;
use conglobate::{
    anchor::{FilesystemPin, RecoveryDirectory},
    domain::RecordedDomain,
    filesystem::{Detached, Mount, MountPoint},
    isolation::{
        CommandDevices, CommandNamespaces, CommandRoot, CommandSetup, IdRange, Mapped,
        PinnedNamespace, PrivateMountNamespace, UserNamespace,
    },
    policy::{BoundDisabled, FilesystemTarget, LabelPolicy, PinDirectory},
    process::{ChildReaper, CommandPrivileges, CommandSyscalls, Supervision},
};
use readiness::Readiness;
use std::{
    fs,
    os::unix::{
        fs::{MetadataExt, PermissionsExt},
        process::CommandExt,
    },
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Debug)]
struct Integration;
impl Integration {
    fn run() -> Result<(), Box<dyn std::error::Error>> {
        if !Path::new("/conglobate-disposable-validation-vm").exists() {
            return Err("disposable guest required".into());
        }
        let args: Vec<_> = std::env::args().collect();
        if args.get(1).is_some_and(|a| a == "gated-namespaces") {
            let script = args.get(2).ok_or("script required")?;
            let (scope, root) = conglobate::isolation::GatedCommandSetup::from_input()?.enter()?;
            let mut command = Command::new(std::env::current_exe()?);
            command
                .args(["gated-root", script])
                .env_clear()
                .stdin(root.into_input()?);
            if args.get(3).is_some_and(|arg| arg == "--probe-extra-fd") {
                command.arg("--probe-extra-fd");
            }
            let child = scope.spawn(command)?;
            let status = child
                .wait_until(Instant::now() + Duration::from_secs(30))?
                .status();
            use std::os::unix::process::ExitStatusExt;
            std::process::exit(status.code().unwrap_or(128 + status.signal().unwrap_or(1)));
        }
        if args.get(1).is_some_and(|a| a == "gated-root") {
            let script = args.get(2).ok_or("script required")?;
            let (devices, gate) = conglobate::isolation::GatedRootSetup::from_input()?.into_parts();
            let scope = PrivateMountNamespace::enter()?;
            let root = CommandRoot::from_current_directory(scope, devices)?;
            let filter = CommandSyscalls::compile()?;
            let command =
                root.enter()?
                    .restrict(script, CommandPrivileges::filesystem(), filter)?;
            let deadline = Instant::now() + Duration::from_secs(30);
            if args.get(3).is_some_and(|arg| arg == "--probe-extra-fd") {
                let extra = fs::File::open("/dev/null")?;
                let error = command.start(gate, deadline).unwrap_err();
                assert!(error.to_string().contains("would remain in command init"));
                drop(extra);
                println!("RESTRICTED_COMMAND_EXTRA_DESCRIPTOR_REJECTED_PASS");
                return Ok(());
            }
            command.start(gate, deadline)?;
            unreachable!();
        }
        if args.get(1).is_some_and(|a| a == "volume-receiver") {
            let deadline = Instant::now() + Duration::from_secs(5);
            let channel = Supervision::from_stdin()?
                .exchange_until(deadline)?
                .join_domain_until(deadline)?;
            let input = channel.receive_volume_until(deadline)?;
            let mode = args.get(2).ok_or("volume command mode required")?;
            let scope = PrivateMountNamespace::enter()?;
            let (view, setup) = input.prepare(&scope, CommandDevices::create(100000, 100000)?)?;
            if mode == "setup-detached-failure" {
                fs::write("/work/setup-failure-stage", "detached")?;
                return Err("injected failure after detached root preparation".into());
            }
            let view = view
                .attach(MountPoint::create(Path::new("/work"))?)
                .map_err(|e| e.to_string())?;
            assert_eq!(view.root_metadata()?.uid(), 100000);
            if mode == "setup-attached-exit" {
                fs::write("/work/setup-failure-stage", "attached")?;
                // Fixture fault injection: no Rust destructors run in this helper.
                std::process::exit(73);
            }
            let (gate, endpoint) = conglobate::isolation::ExecutionGate::pair()?;
            let mut command = Command::new(std::env::current_exe()?);
            command
                .arg("gated-namespaces")
                .arg(Self::command_script(if mode == "policy-cleanup" {
                    "shell-failure"
                } else {
                    mode
                }))
                .stdin(setup.with_gate(endpoint).into_input()?);
            if mode == "descriptor-rejection" {
                command.arg("--probe-extra-fd");
            }
            let running = view.launch(command).map_err(|e| e.to_string())?;
            let gate = gate.wait_ready_until(deadline)?;
            println!("RESTRICTED_LEAF_READY_BEFORE_ADMISSION_PASS");
            channel.report_prepared_until(deadline)?;
            let channel = channel.await_admission_until(deadline)?;
            gate.start_until(deadline)?;
            let (view, status) = running
                .wait_until(Instant::now() + Duration::from_secs(30))
                .map_err(|e| e.to_string())?;
            assert_eq!(
                status.code(),
                Some(if matches!(mode, "shell-failure" | "policy-cleanup") {
                    37
                } else {
                    0
                })
            );
            channel.report_completion_until(status, deadline)?;
            view.close().map_err(|e| e.to_string())?;
            channel.wait_for_loss_until(Instant::now() + Duration::from_secs(5))?;
            scope.restore().map_err(|e| e.to_string())?;
            return Ok(());
        }
        if args
            .get(1)
            .is_some_and(|a| a == "volume-probe" || a.starts_with("volume-supervisor"))
        {
            use conglobate::storage::{Created, Mapping, Unmounted, Volume, Workspace};
            let setup_budget = conglobate::step::SetupBudget::begin();
            use std::{collections::BTreeSet, num::NonZeroU64};
            let points = || -> std::io::Result<BTreeSet<std::path::PathBuf>> {
                fs::read_dir("/work")?
                    .filter_map(|entry| match entry {
                        Ok(entry)
                            if entry
                                .file_name()
                                .as_encoded_bytes()
                                .starts_with(b"conglobate-volume-") =>
                        {
                            Some(Ok(entry.path()))
                        }
                        Ok(_) => None,
                        Err(error) => Some(Err(error)),
                    })
                    .collect()
            };
            assert!(points()?.is_empty());
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
            let id = snapshot.id();
            let node = Path::new("/dev/conglobate-volume-probe");
            rustix::fs::mknodat(
                rustix::fs::CWD,
                node,
                rustix::fs::FileType::BlockDevice,
                rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
                rustix::fs::makedev(id.major(), id.minor()),
            )?;
            let recovery = Path::new("/work/volume-policy-records");
            fs::create_dir(recovery)?;
            fs::set_permissions(recovery, fs::Permissions::from_mode(0o700))?;
            let lease = RecoveryDirectory::open(recovery)?;
            let mut creator = Command::new(std::env::current_exe()?);
            creator.arg("namespace-seed").env_clear();
            let user = UserNamespace::<Mapped>::create(
                creator,
                IdRange::new(100000, 65536)?,
                IdRange::new(100000, 65536)?,
                Instant::now() + Duration::from_secs(5),
            )?
            .pin(&lease)?;
            let scope = if args[1] == "volume-probe" {
                Some(PrivateMountNamespace::enter()?)
            } else {
                None
            };
            let volume = match scope.as_ref() {
                Some(scope) => Volume::<Unmounted>::new(snapshot, scope),
                None => Volume::<Unmounted>::in_init_namespace(snapshot, &lease)
                    .map_err(|e| e.to_string())?,
            };
            let (error, volume) = volume
                .mount_ext4(Path::new("/work"))
                .unwrap_err()
                .into_parts();
            assert_eq!(error.raw_os_error(), Some(22));
            assert!(points()?.is_empty());
            // Fixture-only formatting; production typed formatter is an M5 task.
            assert!(
                Command::new("mke2fs")
                    .args(["-q", "-t", "ext4", "-F"])
                    .arg(node)
                    .status()?
                    .success()
            );
            let volume = volume
                .mount_ext4(Path::new("/work"))
                .map_err(|e| e.to_string())?;
            assert!(volume.root_metadata()?.is_dir());
            let paths = points()?;
            assert_eq!(paths.len(), 1);
            let point = paths.first().unwrap();
            fs::write(point.join("persisted"), b"owned mounted volume")?;
            let pin = if scope.is_none() {
                Some(volume.pin(&lease)?)
            } else {
                None
            };
            if let Some(pin) = &pin {
                assert_eq!(
                    (pin.metadata()?.dev(), pin.metadata()?.ino()),
                    (volume.root_metadata()?.dev(), volume.root_metadata()?.ino())
                );
            }
            let (volume, user, pin) = if scope.is_none() {
                // Fixture-only population; production COPY remains separate.
                assert!(
                    Command::new("cp")
                        .args(["-a", "/image-root/."])
                        .arg(point)
                        .status()?
                        .success()
                );
                let mode = match args[1].as_str() {
                    "volume-supervisor-timeout" => "timeout",
                    "volume-supervisor-failure" => "shell-failure",
                    "volume-supervisor-pre-admission" => "pre-admission",
                    "volume-supervisor-admission-timeout" => "admission-timeout",
                    "volume-supervisor-setup-detached" => "setup-detached-failure",
                    "volume-supervisor-setup-attached" => "setup-attached-exit",
                    "volume-supervisor-vm-death" => "vm-death",
                    "volume-supervisor-policy-cleanup" => "policy-cleanup",
                    "volume-supervisor-descriptor-rejection" => "descriptor-rejection",
                    _ => "success",
                };
                let reaper = ChildReaper::install()?;
                let policy = PolicyFixture::prepare(&lease, user, pin.expect("init volume pin"))?;
                let parent = Path::new("/work/cgroups");
                let domain = RecordedDomain::create(&lease, parent)?;
                let mut command = Command::new(std::env::current_exe()?);
                command.arg("volume-receiver").arg(mode).env_clear();
                let deadline = setup_budget.deadline();
                let helper = domain
                    .launch(command, deadline)
                    .map_err(|e| e.to_string())?;
                let step = conglobate::step::Step::new(policy, helper, reaper)
                    .map_err(|e| e.to_string())?
                    .with_volume(volume);
                step.send_volume_until(deadline)?;
                let step = match mode {
                    "pre-admission" => step.teardown(parent).map_err(|e| e.to_string())?,
                    "setup-detached-failure" | "setup-attached-exit" => {
                        let failure = step.admit_until(deadline).unwrap_err();
                        let conglobate::step::StartFailure::Admission(failure) = failure else {
                            return Err(failure.to_string().into());
                        };
                        let (error, step) = failure.into_parts();
                        assert_ne!(error.kind(), std::io::ErrorKind::TimedOut);
                        assert_eq!(
                            fs::read_to_string("/work/setup-failure-stage")?,
                            if mode == "setup-detached-failure" {
                                "detached"
                            } else {
                                "attached"
                            }
                        );
                        fs::remove_file("/work/setup-failure-stage")?;
                        step.teardown(parent).map_err(|e| e.to_string())?
                    }
                    "admission-timeout" => {
                        let failure = step.admit_until(Instant::now()).unwrap_err();
                        let conglobate::step::StartFailure::Admission(failure) = failure else {
                            return Err(failure.to_string().into());
                        };
                        let (error, step) = failure.into_parts();
                        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
                        step.teardown(parent).map_err(|e| e.to_string())?
                    }
                    _ => {
                        let prepared = setup_budget
                            .await_prepared(step)
                            .map_err(|e| e.to_string())?;
                        let timeout = if mode == "timeout" { "200ms" } else { "5s" };
                        let step = prepared
                            .start(timeout.parse()?)
                            .map_err(|e| e.to_string())?;
                        let deadline = step.deadline();
                        if mode == "vm-death" {
                            let started = point.join("command-started");
                            Readiness::new(&started)?
                                .wait_until(deadline, || step.supervision().peer_lost())?;
                            fs::write("/work/supervisor-ready", "running\n")?;
                            step.supervision()
                                .wait_for_loss_until(Instant::now() + Duration::from_secs(30))?;
                            return Err(
                                "owned volume command ended before supervisor-death exercise"
                                    .into(),
                            );
                        }
                        if mode == "timeout" {
                            let started = point.join("command-started");
                            Readiness::new(&started)?
                                .wait_until(deadline, || step.supervision().peer_lost())?;
                        }
                        let observed = step.observe();
                        match observed.outcome() {
                            conglobate::step::CommandOutcome::Exited(status) => {
                                assert_ne!(mode, "timeout");
                                assert_eq!(
                                    status.code(),
                                    Some(if matches!(mode, "shell-failure" | "policy-cleanup") {
                                        37
                                    } else {
                                        0
                                    })
                                );
                            }
                            conglobate::step::CommandOutcome::ObservationFailed(error) => {
                                assert_eq!(mode, "timeout");
                                assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
                            }
                        }
                        let cleaned = if mode == "policy-cleanup" {
                            let pins =
                                fs::read_dir("/work/pins")?.collect::<Result<Vec<_>, _>>()?;
                            assert_eq!(pins.len(), 1);
                            let blocker = pins[0].path().join("cleanup-blocker");
                            fs::create_dir(&blocker)?;
                            let failed = observed.teardown(parent);
                            let failure = failed.cleanup().as_ref().unwrap_err();
                            assert_eq!(
                                failure.diagnostics().operation().kind(),
                                std::io::ErrorKind::DirectoryNotEmpty
                            );
                            assert_eq!(fs::read_dir(recovery)?.count(), 2);
                            fs::remove_dir(&blocker)?;
                            let completion =
                                failed.map_cleanup(|result| result.unwrap_err().retry());
                            println!("BOUNDED_POLICY_CLEANUP_RETRY_PASS");
                            println!("NONZERO_OUTCOME_THROUGH_CLEANUP_RETRY_PASS");
                            completion
                        } else {
                            observed.teardown(parent)
                        };
                        let (outcome, cleanup) = cleaned.into_parts();
                        assert_eq!(
                            outcome.success(),
                            !matches!(mode, "timeout" | "shell-failure" | "policy-cleanup")
                        );
                        println!("PRIMARY_OUTCOME_AFTER_CLEANUP_PASS");
                        cleanup.map_err(|e| e.to_string())?
                    }
                };
                assert_eq!(fs::read_dir(recovery)?.count(), 0);
                assert_eq!(fs::read_dir("/work/pins")?.count(), 0);
                assert!(
                    fs::read_to_string("/proc/thread-self/children")?
                        .trim()
                        .is_empty()
                );
                if matches!(
                    mode,
                    "pre-admission"
                        | "admission-timeout"
                        | "setup-detached-failure"
                        | "setup-attached-exit"
                        | "descriptor-rejection"
                ) {
                    assert!(!point.join("command-started").exists());
                    assert!(!point.join("labelled").exists());
                } else {
                    assert_eq!(
                        fs::read_to_string(point.join("command-started"))?,
                        "image-write\n"
                    );
                    assert_eq!(fs::metadata(point.join("command-started"))?.uid(), 0);
                    let mut label = [0u8; 128];
                    let length = rustix::fs::getxattr(
                        point.join("labelled"),
                        "security.selinux",
                        &mut label,
                    )?;
                    assert_eq!(&label[..length], b"system_u:object_r:etc_t:s0");
                }
                println!("OWNED_VOLUME_COMMAND_{}_PASS", mode.to_uppercase());
                println!("OWNED_VOLUME_SUPERVISED_TRANSFER_PASS");
                println!("STEP_RETAINS_VOLUME_THROUGH_QUIESCENCE_PASS");
                let holder = fs::File::open(point)?;
                let failure = step.unmount().unwrap_err();
                assert_eq!(
                    failure.diagnostics().operation().kind(),
                    std::io::ErrorKind::ResourceBusy
                );
                drop(holder);
                let (_, _, volume) = failure.retry().map_err(|e| e.to_string())?.into_parts();
                let volume = volume
                    .mount_ext4(Path::new("/work"))
                    .map_err(|e| e.to_string())?;
                // Recreate fixture-only identities for the independent view tests.
                let mut creator = Command::new(std::env::current_exe()?);
                creator.arg("namespace-seed").env_clear();
                let user = UserNamespace::<Mapped>::create(
                    creator,
                    IdRange::new(100000, 65536)?,
                    IdRange::new(100000, 65536)?,
                    Instant::now() + Duration::from_secs(5),
                )?
                .pin(&lease)?;
                let pin = volume.pin(&lease)?;
                println!("BOUNDED_POLICY_PINS_AND_UNMOUNT_PASS");
                (volume, user, Some(pin))
            } else {
                (volume, user, pin)
            };
            let paths = points()?;
            assert_eq!(paths.len(), 1);
            let point = paths.first().unwrap();
            let view = volume.view()?;
            let command_scope = PrivateMountNamespace::enter()?;
            assert!(volume.pin(&lease).is_err());
            assert!(volume.view().is_err());
            let view = user
                .idmap(view.in_namespace(&command_scope)?)
                .map_err(|e| e.to_string())?
                .attach(MountPoint::create(Path::new("/work"))?)
                .map_err(|e| e.to_string())?;
            assert_eq!(view.root_metadata()?.uid(), 100000);
            view.close().map_err(|e| e.to_string())?;
            println!("OWNED_VOLUME_IDMAPPED_VIEW_PASS");
            command_scope.restore().map_err(|e| e.to_string())?;
            if let Some(pin) = pin {
                pin.release()?;
                println!("OWNED_VOLUME_POLICY_PIN_PASS");
            }
            let holder = fs::File::open(point)?;
            let failure = volume.unmount().unwrap_err();
            assert_eq!(
                failure.source_error().kind(),
                std::io::ErrorKind::ResourceBusy
            );
            let wrong_scope = PrivateMountNamespace::enter()?;
            let failure = failure.retry().unwrap_err();
            assert!(failure.source_error().to_string().contains("namespace"));
            assert_eq!(
                failure.diagnostics().operation().kind(),
                std::io::ErrorKind::ResourceBusy
            );
            assert_eq!(failure.diagnostics().cleanup().len(), 1);
            let failure = failure.retry().unwrap_err();
            assert_eq!(
                failure.diagnostics().operation().kind(),
                std::io::ErrorKind::ResourceBusy
            );
            assert_eq!(failure.diagnostics().cleanup().len(), 2);
            assert!(
                failure
                    .diagnostics()
                    .cleanup()
                    .iter()
                    .all(|error| error.to_string().contains("namespace"))
            );
            println!("VOLUME_CLEANUP_DIAGNOSTICS_RETAINED_PASS");
            wrong_scope.restore().map_err(|e| e.to_string())?;
            drop(holder);
            let volume = failure.retry().map_err(|e| e.to_string())?;
            assert!(points()?.is_empty());
            let volume = volume
                .mount_ext4(Path::new("/work"))
                .map_err(|e| e.to_string())?;
            let paths = points()?;
            assert_eq!(paths.len(), 1);
            assert_eq!(
                fs::read(paths.first().unwrap().join("persisted"))?,
                b"owned mounted volume"
            );
            let volume = volume.unmount().map_err(|e| e.to_string())?;
            assert!(points()?.is_empty());
            let snapshot = volume.into_snapshot();
            snapshot.usage()?;
            snapshot
                .close()
                .map_err(|e| e.to_string())?
                .detach()
                .map_err(|e| e.to_string())?;
            origin.close()?;
            if let Some(scope) = scope {
                scope.restore().map_err(|e| e.to_string())?;
            }
            user.release()?;
            drop(lease);
            fs::remove_dir(recovery)?;
            fs::remove_file(node)?;
            println!("OWNED_VOLUME_MOUNT_RETRY_AND_REMOUNT_PASS");
            return Ok(());
        }
        if args.get(1).is_some_and(|a| a == "assemble-chain") {
            use std::{
                collections::BTreeSet,
                io::{Read, Seek, SeekFrom, Write},
                num::NonZeroU64,
            };
            let mappings = || -> std::io::Result<BTreeSet<std::ffi::OsString>> {
                fs::read_dir("/sys/block")?
                    .filter_map(|entry| match entry {
                        Ok(entry) if entry.file_name().as_encoded_bytes().starts_with(b"dm-") => {
                            Some(Ok(entry.file_name()))
                        }
                        Ok(_) => None,
                        Err(error) => Some(Err(error)),
                    })
                    .collect()
            };
            let baseline = mappings()?;
            let workspace = conglobate::storage::Workspace::create()?;
            let cache = conglobate::cache::Cache::open(Path::new("/cache-fixture"))?;
            for name in ["chain", "invalid-header"] {
                let pin = fs::read_to_string(format!("/cache-fixture/{name}.pin"))?;
                let image = cache.resolve(pin.parse()?)?;
                let chain = cache.verify_chain(&image)?.ok_or("missing chain")?;
                if name == "invalid-header" {
                    let failure = chain.assemble(&cache, &workspace).unwrap_err();
                    assert_eq!(
                        failure.source_error().kind(),
                        std::io::ErrorKind::InvalidData
                    );
                    failure.cleanup().map_err(|e| e.to_string())?;
                    assert_eq!(mappings()?, baseline);
                    println!("INVALID_SECOND_SCUTE_CLEANUP_PASS");
                    continue;
                }
                let mounted = chain
                    .assemble(&cache, &workspace)
                    .map_err(|e| e.to_string())?;
                assert_eq!(mounted.root(), chain.root());
                let id = mounted.id();
                let node = Path::new("/dev/conglobate-chain-probe");
                rustix::fs::mknodat(
                    rustix::fs::CWD,
                    node,
                    rustix::fs::FileType::BlockDevice,
                    rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
                    rustix::fs::makedev(id.major(), id.minor()),
                )?;
                if let Ok(mut writer) = fs::OpenOptions::new().write(true).open(node) {
                    assert!(writer.write_all(&[0; 4096]).is_err());
                }
                let mut reader = fs::File::open(node)?;
                let mut bytes = [0; 12288];
                reader.read_exact(&mut bytes)?;
                assert_eq!(&bytes[..4096], &[66; 4096]);
                assert_eq!(&bytes[4096..8192], &[67; 4096]);
                assert_eq!(&bytes[8192..], &[0; 4096]);
                let inherited = bytes;
                let mut scratch = workspace.scratch()?;
                scratch.set_len(1024 * 1024)?;
                let sectors = NonZeroU64::new(8192).unwrap();
                let snapshot =
                    conglobate::storage::Mapping::<conglobate::storage::Created>::create()?
                        .snapshot(
                            &mounted,
                            scratch.attach().map_err(|e| e.to_string())?,
                            sectors,
                        )
                        .map_err(|e| e.to_string())?;
                let snapshot_node = Path::new("/dev/conglobate-chain-writable");
                let id = snapshot.id();
                rustix::fs::mknodat(
                    rustix::fs::CWD,
                    snapshot_node,
                    rustix::fs::FileType::BlockDevice,
                    rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
                    rustix::fs::makedev(id.major(), id.minor()),
                )?;
                let mut writer = fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(snapshot_node)?;
                writer.read_exact(&mut bytes)?;
                assert_eq!(bytes, inherited);
                writer.rewind()?;
                writer.write_all(&[99; 4096])?;
                writer.seek(SeekFrom::Start(8192))?;
                writer.write_all(&[77; 4096])?;
                writer.sync_all()?;
                snapshot.usage()?;
                reader.rewind()?;
                reader.read_exact(&mut bytes)?;
                assert_eq!(bytes, inherited);
                let failure = snapshot.close().unwrap_err();
                assert_eq!(
                    failure.source_error().kind(),
                    std::io::ErrorKind::ResourceBusy
                );
                drop(writer);
                let backing = failure.cleanup().map_err(|e| e.to_string())?;
                let scratch = backing.detach().map_err(|e| e.to_string())?;
                fs::remove_file(snapshot_node)?;
                // Reopen the persistent store against the same unchanged chain.
                let snapshot =
                    conglobate::storage::Mapping::<conglobate::storage::Created>::create()?
                        .snapshot(
                            &mounted,
                            scratch.attach().map_err(|e| e.to_string())?,
                            sectors,
                        )
                        .map_err(|e| e.to_string())?;
                let id = snapshot.id();
                rustix::fs::mknodat(
                    rustix::fs::CWD,
                    snapshot_node,
                    rustix::fs::FileType::BlockDevice,
                    rustix::fs::Mode::RUSR,
                    rustix::fs::makedev(id.major(), id.minor()),
                )?;
                let mut changed = fs::File::open(snapshot_node)?;
                changed.read_exact(&mut bytes)?;
                assert_eq!(&bytes[..4096], &[99; 4096]);
                assert_eq!(&bytes[4096..8192], &[67; 4096]);
                assert_eq!(&bytes[8192..], &[77; 4096]);
                drop(changed);
                snapshot
                    .close()
                    .map_err(|e| e.to_string())?
                    .detach()
                    .map_err(|e| e.to_string())?;
                fs::remove_file(snapshot_node)?;
                reader.rewind()?;
                reader.read_exact(&mut bytes)?;
                assert_eq!(bytes, inherited);
                assert_eq!(cache.verify_chain(&image)?.unwrap().root(), chain.root());
                println!("WRITABLE_SNAPSHOT_OVER_VERIFIED_CHAIN_PASS");
                let failure = mounted.close().unwrap_err();
                assert_eq!(
                    failure.source_error().kind(),
                    std::io::ErrorKind::ResourceBusy
                );
                let failure = failure.cleanup().unwrap_err();
                assert_eq!(failure.diagnostics().operation().raw_os_error(), Some(16));
                assert_eq!(failure.diagnostics().cleanup().len(), 1);
                assert_eq!(failure.diagnostics().cleanup()[0].raw_os_error(), Some(16));
                assert_eq!(failure.to_string().matches("cleanup attempt").count(), 1);
                drop(reader);
                failure.cleanup().map_err(|e| e.to_string())?;
                println!("CHAIN_CLEANUP_DIAGNOSTICS_RETAINED_PASS");
                fs::remove_file(node)?;
                assert_eq!(mappings()?, baseline);
            }
            println!("OWNED_TWO_SCUTE_CHAIN_PASS");
            return Ok(());
        }
        if args.get(1).is_some_and(|a| a == "prepare-scutes") {
            use std::io::{Read, Write};
            let workspace = conglobate::storage::Workspace::create()?;
            let cache = conglobate::cache::Cache::open(Path::new("/cache-fixture"))?;
            for name in ["aligned", "partial", "compressed"] {
                let pin = fs::read_to_string(format!("/cache-fixture/{name}.pin"))?;
                let image = cache.resolve(pin.parse()?)?;
                let chain = cache.verify_chain(&image)?.ok_or("missing scutes")?;
                let scute = &chain.scutes()[0];
                let prepared = scute.prepare(&cache, &workspace)?;
                assert_eq!(prepared.is_cache_backed(), name == "aligned");
                assert_eq!(prepared.root(), chain.root());
                assert_eq!(prepared.data_size()?, 8192);
                assert_eq!(prepared.hash_size()?, 8192);
                let path = format!("/cache-fixture/blobs/sha256/{}", scute.digest().hex());
                let original = fs::read(&path)?;
                let verified = scute
                    .prepare(&cache, &workspace)?
                    .activate()
                    .map_err(|e| e.to_string())?;
                let id = verified.id();
                let node = Path::new("/dev/conglobate-verity-probe");
                rustix::fs::mknodat(
                    rustix::fs::CWD,
                    node,
                    rustix::fs::FileType::BlockDevice,
                    rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
                    rustix::fs::makedev(id.major(), id.minor()),
                )?;
                if let Ok(mut writer) = fs::OpenOptions::new().write(true).open(node) {
                    assert!(writer.write_all(&[0; 4096]).is_err());
                }
                let mut holder = fs::File::open(node)?;
                let mut data = vec![0; 8192];
                holder.read_exact(&mut data)?;
                let actual_size = scute.extent().size() as usize;
                assert!(data[..actual_size].iter().all(|byte| *byte == 42));
                assert!(data[actual_size..].iter().all(|byte| *byte == 0));
                let failure = verified.close().unwrap_err();
                assert_eq!(
                    failure.source_error().kind(),
                    std::io::ErrorKind::ResourceBusy
                );
                let failure = failure.cleanup().unwrap_err();
                let failure = failure.cleanup().unwrap_err();
                assert_eq!(failure.diagnostics().operation().raw_os_error(), Some(16));
                assert_eq!(failure.diagnostics().cleanup().len(), 2);
                assert!(
                    failure
                        .diagnostics()
                        .cleanup()
                        .iter()
                        .all(|error| error.raw_os_error() == Some(16))
                );
                assert_eq!(failure.to_string().matches("cleanup attempt").count(), 2);
                drop(holder);
                failure.cleanup().map_err(|e| e.to_string())?;
                println!("VERITY_CLEANUP_DIAGNOSTICS_RETAINED_PASS");
                fs::remove_file(node)?;
                if name == "aligned" {
                    // Fresh mapping: corrupt after activation but before its first
                    // read, so a previously verified page cannot mask the test.
                    let verified = scute
                        .prepare(&cache, &workspace)?
                        .activate()
                        .map_err(|e| e.to_string())?;
                    let id = verified.id();
                    rustix::fs::mknodat(
                        rustix::fs::CWD,
                        node,
                        rustix::fs::FileType::BlockDevice,
                        rustix::fs::Mode::RUSR,
                        rustix::fs::makedev(id.major(), id.minor()),
                    )?;
                    let mut corruptor = fs::OpenOptions::new().write(true).open(&path)?;
                    corruptor.write_all(&[1; 4096])?;
                    corruptor.sync_all()?;
                    drop(corruptor);
                    let mut holder = fs::File::open(node)?;
                    let error = holder.read_exact(&mut [0; 4096]).unwrap_err();
                    assert_eq!(error.raw_os_error(), Some(5));
                    drop(holder);
                    verified.close().map_err(|e| e.to_string())?;
                    fs::remove_file(node)?;
                    fs::write(&path, &original)?;
                    println!("VERITY_CACHE_CORRUPTION_READ_REJECTED_PASS");
                }
                fs::write(&path, vec![1; original.len()])?;
                assert!(scute.prepare(&cache, &workspace).is_err());
                fs::write(&path, &original)?;
                if name == "aligned" {
                    fs::rename(&path, format!("{path}.original"))?;
                    fs::write(&path, b"replacement")?;
                    assert_eq!(prepared.data_size()?, 8192);
                    assert!(scute.prepare(&cache, &workspace).is_err());
                }
            }
            println!("CACHE_BACKED_SCUTE_PREPARATION_PASS");
            println!("OWNED_VERITY_READ_AND_CLEANUP_PASS");
            return Ok(());
        }
        if args.get(1).is_some_and(|a| a == "wait-marker") {
            let marker = args.get(2).ok_or("marker path required")?;
            Readiness::new(Path::new(marker))?
                .wait_until(Instant::now() + Duration::from_secs(5), || Ok(false))?;
            return Ok(());
        }
        if args.get(1).is_some_and(|a| a == "snapshot-probe") {
            use conglobate::storage::{Created, Mapping};
            use std::{
                io::{Read, Write},
                num::NonZeroU64,
            };
            let workspace = conglobate::storage::Workspace::create()?;
            let mut scratch = workspace.scratch()?;
            scratch.set_len(16 * 1024 * 1024)?;
            let sectors = NonZeroU64::new(8192).unwrap();
            let origin = Mapping::<Created>::create()?.zero(sectors)?;
            let backing = scratch.attach().map_err(|e| e.to_string())?;
            let snapshot = Mapping::<Created>::create()?
                .snapshot(&origin, backing, sectors)
                .map_err(|e| e.to_string())?;
            let id = snapshot.id();
            let path = Path::new("/dev/conglobate-snapshot-probe");
            rustix::fs::mknodat(
                rustix::fs::CWD,
                path,
                rustix::fs::FileType::BlockDevice,
                rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
                rustix::fs::makedev(id.major(), id.minor()),
            )?;
            let mut holder = fs::OpenOptions::new().read(true).write(true).open(path)?;
            let before_usage = snapshot.usage()?;
            holder.write_all(b"snapshot data")?;
            holder.sync_all()?;
            assert!(snapshot.usage()?.allocated_sectors() >= before_usage.allocated_sectors());
            let failure = snapshot.close().unwrap_err();
            assert_eq!(failure.source_error().raw_os_error(), Some(16));
            drop(holder);
            let backing = failure.cleanup().map_err(|e| e.to_string())?;
            origin.close()?;
            fs::remove_file(path)?;
            let scratch = backing.detach().map_err(|e| e.to_string())?;
            // Reactivate the persisted exception store over a fresh dm-zero.
            let origin = Mapping::<Created>::create()?.zero(sectors)?;
            let backing = scratch.attach().map_err(|e| e.to_string())?;
            let snapshot = Mapping::<Created>::create()?
                .snapshot(&origin, backing, sectors)
                .map_err(|e| e.to_string())?;
            let id = snapshot.id();
            rustix::fs::mknodat(
                rustix::fs::CWD,
                path,
                rustix::fs::FileType::BlockDevice,
                rustix::fs::Mode::RUSR,
                rustix::fs::makedev(id.major(), id.minor()),
            )?;
            let mut holder = fs::File::open(path)?;
            let mut bytes = [0; 13];
            holder.read_exact(&mut bytes)?;
            assert_eq!(&bytes, b"snapshot data");
            drop(holder);
            let backing = snapshot.close().map_err(|e| e.to_string())?;
            origin.close()?;
            backing.detach().map_err(|e| e.to_string())?;
            fs::remove_file(path)?;
            let mut scratch = workspace.scratch()?;
            scratch.set_len(64 * 1024)?;
            let origin = Mapping::<Created>::create()?.zero(sectors)?;
            let backing = scratch.attach().map_err(|e| e.to_string())?;
            let snapshot = Mapping::<Created>::create()?
                .snapshot(&origin, backing, sectors)
                .map_err(|e| e.to_string())?;
            snapshot.usage()?;
            let id = snapshot.id();
            rustix::fs::mknodat(
                rustix::fs::CWD,
                path,
                rustix::fs::FileType::BlockDevice,
                rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
                rustix::fs::makedev(id.major(), id.minor()),
            )?;
            let mut holder = fs::OpenOptions::new().read(true).write(true).open(path)?;
            let write = holder.write_all(&vec![42; 128 * 1024]);
            let sync = holder.sync_all();
            assert!(write.is_err() || sync.is_err());
            assert_eq!(
                snapshot.usage().unwrap_err().kind(),
                std::io::ErrorKind::InvalidData
            );
            drop(holder);
            let backing = snapshot.close().map_err(|e| e.to_string())?;
            origin.close()?;
            backing.detach().map_err(|e| e.to_string())?;
            fs::remove_file(path)?;
            println!("SNAPSHOT_EXHAUSTION_REJECTS_VALID_USAGE_PASS");
            println!("OWNED_PERSISTENT_SNAPSHOT_REACTIVATION_PASS");
            return Ok(());
        }
        if args.get(1).is_some_and(|a| a == "loop-probe") {
            use std::io::{Read, Seek, Write};
            let workspace = conglobate::storage::Workspace::create()?;
            let empty = workspace.scratch()?;
            let failure = empty.attach().unwrap_err();
            let (_, mut scratch) = failure.into_parts();
            assert_eq!(scratch.metadata()?.len(), 0);
            scratch.set_len(1024 * 1024)?;
            let mut attachment = scratch.attach().map_err(|error| error.to_string())?;
            attachment.write_all(b"loop data")?;
            let holder = fs::File::open(format!("/dev/loop{}", attachment.number()?))?;
            let failure = attachment.detach().unwrap_err();
            assert_eq!(
                failure.source_error().kind(),
                std::io::ErrorKind::ResourceBusy
            );
            drop(holder);
            let mut scratch = failure.retry().map_err(|error| error.to_string())?;
            scratch.rewind()?;
            let mut bytes = [0; 9];
            scratch.read_exact(&mut bytes)?;
            assert_eq!(&bytes, b"loop data");
            // Returned backing authority is writable only after confirmed detach.
            scratch.rewind()?;
            scratch.write_all(b"file data")?;
            let mut attachment = scratch.attach().map_err(|error| error.to_string())?;
            attachment.read_exact(&mut bytes)?;
            assert_eq!(&bytes, b"file data");
            attachment.detach().map_err(|error| error.to_string())?;
            println!("RAM_LOOP_DEFERRED_DETACH_AND_BACKING_RETURN_PASS");
            return Ok(());
        }
        if args.get(1).is_some_and(|a| a == "mapping-probe") {
            use conglobate::storage::{Created, Mapping};
            use std::{io::Read, num::NonZeroU64};
            let first = Mapping::<Created>::create()?;
            let second = Mapping::<Created>::create()?;
            assert_ne!(first.id(), second.id());
            second.close()?;
            let active = first.zero(NonZeroU64::new(8192).unwrap())?;
            assert_eq!(active.status()?.target_count, 1);
            let id = active.id();
            let path = Path::new("/dev/conglobate-zero-probe");
            rustix::fs::mknodat(
                rustix::fs::CWD,
                path,
                rustix::fs::FileType::BlockDevice,
                rustix::fs::Mode::RUSR,
                rustix::fs::makedev(id.major(), id.minor()),
            )?;
            let mut holder = fs::File::open(path)?;
            let mut bytes = [1u8; 4096];
            holder.read_exact(&mut bytes)?;
            assert_eq!(bytes, [0; 4096]);
            let failure = active.close().unwrap_err();
            assert_eq!(failure.source_error().raw_os_error(), Some(16));
            let failure = failure.cleanup().unwrap_err();
            assert_eq!(failure.diagnostics().operation().raw_os_error(), Some(16));
            assert_eq!(failure.diagnostics().cleanup().len(), 1);
            assert_eq!(failure.diagnostics().cleanup()[0].raw_os_error(), Some(16));
            assert!(failure.to_string().contains("cleanup attempt 1:"));
            drop(holder);
            failure.cleanup()?;
            println!("MAPPING_CLEANUP_DIAGNOSTICS_RETAINED_PASS");
            assert!(fs::File::open(path).is_err());
            fs::remove_file(path)?;
            let empty = Mapping::<Created>::create()?;
            let failure = empty.zero(NonZeroU64::new(u64::MAX).unwrap()).unwrap_err();
            assert_eq!(
                failure.source_error().kind(),
                std::io::ErrorKind::InvalidInput
            );
            failure.cleanup()?;
            println!("OWNED_DM_ZERO_BUSY_AND_SETUP_CLEANUP_PASS");
            return Ok(());
        }
        if args.get(1).is_some_and(|a| a == "workspace-probe") {
            use std::io::{Read, Seek, SeekFrom, Write};
            let before = fs::read_to_string("/proc/self/mountinfo")?;
            let workspace = conglobate::storage::Workspace::create()?;
            let mut first = workspace.scratch()?;
            let mut second = workspace.scratch()?;
            assert_eq!(first.metadata()?.nlink(), 0);
            assert_eq!(first.metadata()?.mode() & 0o777, 0o600);
            assert_eq!(first.metadata()?.uid(), 0);
            assert_eq!(first.metadata()?.dev(), second.metadata()?.dev());
            assert_ne!(first.metadata()?.ino(), second.metadata()?.ino());
            first.set_len(1024 * 1024)?;
            first.seek(SeekFrom::Start(4096))?;
            first.write_all(b"workspace")?;
            first.sync_all()?;
            first.rewind()?;
            let mut hole = [1u8; 4096];
            first.read_exact(&mut hole)?;
            assert_eq!(hole, [0; 4096]);
            let mut data = [0; 9];
            first.read_exact(&mut data)?;
            assert_eq!(&data, b"workspace");
            assert_eq!(second.read(&mut data)?, 0);
            second.write_all(b"independent")?;
            let commit = fs::read_to_string("/workspace-context-commit")?;
            let context = conglobate::context::CapturedContext::capture(
                Path::new("/workspace-context"),
                &conglobate::request::CommitIdentity::try_from(commit.trim())?,
                &conglobate::path::ContextDirectory::try_from(Path::new("."))?,
            )?;
            let failed_staging = workspace.materialize(&context)?;
            let limit = rustix::process::getrlimit(rustix::process::Resource::Nofile);
            rustix::process::setrlimit(
                rustix::process::Resource::Nofile,
                rustix::process::Rlimit {
                    current: Some(0),
                    maximum: limit.maximum,
                },
            )?;
            let failed = failed_staging.read_only();
            // Restore before checking the failure, including an unexpected success.
            rustix::process::setrlimit(rustix::process::Resource::Nofile, limit)?;
            let failure = failed.unwrap_err();
            assert_eq!(failure.source_error().raw_os_error(), Some(24));
            let (_, staged_retry) = failure.into_parts();
            staged_retry.read_only()?.close()?;
            println!("DETACHED_CONTEXT_FAILED_SETUP_RETRY_PASS");
            let staged = workspace.materialize(&context)?;
            let path = conglobate::path::ContextPath::try_from(Path::new("input"))?;
            let mut reader = staged.open(&path)?;
            assert_eq!(reader.metadata()?.dev(), first.metadata()?.dev());
            let mut input = String::new();
            reader.read_to_string(&mut input)?;
            assert_eq!(input, "committed context\n");
            drop(reader);
            drop(first);
            drop(second);
            drop(workspace);
            // Its owned parent descriptor retains tmpfs even after the initial
            // workspace owner is gone. Cleanup must still resolve and succeed.
            let mut reader = staged.open(&path)?;
            input.clear();
            reader.read_to_string(&mut input)?;
            assert_eq!(input, "committed context\n");
            drop(reader);
            let readonly = staged.read_only()?;
            let mut file = readonly.open(&path)?;
            assert_eq!(
                file.set_permissions(fs::Permissions::from_mode(0o600))
                    .unwrap_err()
                    .raw_os_error(),
                Some(30)
            );
            let limit = rustix::process::getrlimit(rustix::process::Resource::Nofile);
            rustix::process::setrlimit(
                rustix::process::Resource::Nofile,
                rustix::process::Rlimit {
                    current: Some(0),
                    maximum: limit.maximum,
                },
            )?;
            let cleanup = readonly.close();
            rustix::process::setrlimit(rustix::process::Resource::Nofile, limit)?;
            let cleanup = cleanup.unwrap_err();
            assert_eq!(cleanup.source_error().raw_os_error(), Some(24));
            cleanup.retry()?;
            println!("CONTEXT_CLEANUP_FAILURE_RETAINS_PARENT_PASS");
            input.clear();
            file.read_to_string(&mut input)?;
            assert_eq!(input, "committed context\n");
            assert_eq!(
                file.set_permissions(fs::Permissions::from_mode(0o600))
                    .unwrap_err()
                    .raw_os_error(),
                Some(30)
            );
            drop(file);
            println!("DETACHED_READONLY_CONTEXT_AND_READER_LIFETIME_PASS");
            println!("WORKSPACE_CONTEXT_PARENT_LIFETIME_PASS");
            assert_eq!(fs::read_to_string("/proc/self/mountinfo")?, before);
            println!("DETACHED_RAM_WORKSPACE_SCRATCH_PASS");
            return Ok(());
        }
        if args.get(1).is_some_and(|a| a == "filesystem-pin-probe") {
            let path = Path::new("/work/filesystem-pin-test");
            fs::create_dir(path)?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
            let lease = RecoveryDirectory::open(path)?;
            assert!(FilesystemPin::create(&lease, fs::File::open("/supervisor-secret")?).is_err());
            assert_eq!(fs::read_dir(path)?.count(), 0);
            let original = fs::metadata("/work/origin")?;
            let pin = FilesystemPin::create(&lease, fs::File::open("/work/origin")?)?;
            assert!(
                Command::new("umount")
                    .arg("/work/origin")
                    .status()?
                    .success()
            );
            let held = pin.metadata()?;
            assert_eq!((held.dev(), held.ino()), (original.dev(), original.ino()));
            // Fixture-only discovery restores its original mount after testing
            // that the production pin alone retains the selected filesystem.
            let entries = fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
            assert_eq!(entries.len(), 1);
            assert!(
                Command::new("mount")
                    .arg("--bind")
                    .arg(entries[0].path())
                    .arg("/work/origin")
                    .status()?
                    .success()
            );
            pin.release()?;
            assert_eq!(fs::read_dir(path)?.count(), 0);
            assert_eq!(fs::metadata("/work/origin")?.dev(), original.dev());
            drop(lease);
            fs::remove_dir(path)?;
            println!("NATIVE_FILESYSTEM_PIN_RETAINS_UNMOUNTED_SOURCE_PASS");
            return Ok(());
        }
        if args.get(1).is_some_and(|a| a == "reaper-probe") {
            let reaper = ChildReaper::install()?;
            let mut child = Command::new("/bin/sh")
                .args(["-c", "read line; exit 0"])
                .stdin(Stdio::piped())
                .spawn()?;
            let release = child.stdin.take().ok_or("missing child release pipe")?;
            // Relinquish individual reaping ownership to the process-wide drain.
            // Child's destructor does not kill or reap it.
            drop(child);
            let failure = reaper.drain_until(Instant::now()).unwrap_err();
            let (error, reaper) = failure.into_parts();
            assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
            drop(release);
            let children = reaper.drain_until(Instant::now() + Duration::from_secs(5))?;
            assert_eq!(children.collected(), 1);
            assert!(
                fs::read_to_string("/proc/thread-self/children")?
                    .trim()
                    .is_empty()
            );
            println!("NATIVE_REAPER_TIMEOUT_AND_RELEASE_PASS");
            return Ok(());
        }
        if args.get(1).is_some_and(|a| a == "root-launch-probe") {
            use std::os::{fd::AsFd, unix::net::UnixDatagram};
            let selected = fs::metadata(".")?;
            let expected = fs::metadata("/work/origin")?;
            assert_eq!(
                (selected.dev(), selected.ino()),
                (expected.dev(), expected.ino())
            );
            let channel = UnixDatagram::from(std::io::stdin().as_fd().try_clone_to_owned()?);
            channel.set_read_timeout(Some(Duration::from_secs(5)))?;
            assert_eq!(channel.send(b"ready")?, 5);
            let mut reply = [0; 5];
            let count = channel.recv(&mut reply)?;
            assert_eq!(&reply[..count], b"exit");
            return Ok(());
        }
        if args.get(1).is_some_and(|a| a == "namespace-seed") {
            UserNamespace::<Mapped>::serve(Instant::now() + Duration::from_secs(5))?;
            return Ok(());
        }
        if args.get(1).is_some_and(|a| a == "namespace-seed-unchanged") {
            let deadline = Instant::now() + Duration::from_secs(5);
            Supervision::from_stdin()?
                .exchange_until(deadline)?
                .wait_for_loss_until(deadline)?;
            return Ok(());
        }
        if args.get(1).is_some_and(|a| a == "namespace-rejections") {
            let error =
                CommandNamespaces::enter(fs::File::open("/supervisor-secret")?).unwrap_err();
            assert!(error.to_string().contains("requires nsfs"));
            let userns = fs::File::open("/proc/self/ns/user")?;
            let (release, gate) = std::sync::mpsc::channel::<()>();
            let (ready, entered) = std::sync::mpsc::channel::<()>();
            let thread = std::thread::spawn(move || {
                ready.send(()).unwrap();
                gate.recv_timeout(Duration::from_secs(5)).unwrap();
            });
            entered.recv_timeout(Duration::from_secs(5))?;
            let error = CommandNamespaces::enter(userns).unwrap_err();
            release.send(())?;
            thread.join().unwrap();
            assert!(error.to_string().contains("requires one thread"));
            let before = fs::read_to_string("/proc/self/uid_map")?;
            let mut creator = Command::new(std::env::current_exe()?);
            creator.arg("namespace-seed-unchanged").env_clear();
            let deadline = Instant::now() + Duration::from_secs(5);
            let failure = UserNamespace::<Mapped>::create(
                creator,
                IdRange::new(100000, 65536)?,
                IdRange::new(100000, 65536)?,
                deadline,
            )
            .unwrap_err();
            match failure {
                conglobate::isolation::UserNamespaceFailure::Mapping { source, process } => {
                    assert!(
                        source
                            .to_string()
                            .contains("did not create a new user namespace")
                    );
                    assert!(process.wait_until(deadline)?.status().success());
                }
                failure => return Err(failure.into()),
            }
            assert_eq!(fs::read_to_string("/proc/self/uid_map")?, before);
            assert!(
                fs::read_to_string("/proc/thread-self/children")?
                    .trim()
                    .is_empty()
            );
            let scope = PrivateMountNamespace::enter()?;
            let error = PinDirectory::create(Path::new("/work/pins")).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("deployment init's mount/PID namespace")
            );
            scope.restore().map_err(|error| error.to_string())?;
            println!("POLICY_PRIVATE_NAMESPACE_REJECTED_PASS");
            let scope = PrivateMountNamespace::enter()?;
            let foreign_policy = Path::new("/work/foreign-policy-pin-test");
            let foreign_recovery = Path::new("/work/foreign-recovery-pin-test");
            fs::create_dir(foreign_policy)?;
            fs::create_dir(foreign_recovery)?;
            assert!(
                Command::new("mount")
                    .args(["-t", "bpf", "bpf"])
                    .arg(foreign_policy)
                    .status()?
                    .success()
            );
            assert!(
                Command::new("mount")
                    .args(["-t", "tmpfs", "-o", "mode=0700", "tmpfs"])
                    .arg(foreign_recovery)
                    .status()?
                    .success()
            );
            let policy_directory = fs::File::open(foreign_policy)?;
            let recovery_directory = fs::File::open(foreign_recovery)?;
            scope.restore().map_err(|error| error.to_string())?;
            use std::os::fd::AsRawFd;
            let policy_path = format!("/proc/self/fd/{}/.", policy_directory.as_raw_fd());
            let error = PinDirectory::create(Path::new(&policy_path)).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("not attached in the deployment init namespace")
            );
            let recovery_path = format!("/proc/self/fd/{}/.", recovery_directory.as_raw_fd());
            let error = RecoveryDirectory::open(Path::new(&recovery_path)).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("not attached in the deployment init namespace")
            );
            drop(policy_directory);
            drop(recovery_directory);
            fs::remove_dir(foreign_policy)?;
            fs::remove_dir(foreign_recovery)?;
            println!("FOREIGN_CONTROL_MOUNT_DESCRIPTORS_REJECTED_PASS");

            println!("NATIVE_NAMESPACE_SETUP_REJECTIONS_PASS");
            return Ok(());
        }
        if args.get(1).is_some_and(|a| a == "native-namespaces") {
            let script = args.get(2).ok_or("script required")?;
            let input = CommandSetup::from_input()?;
            let expected_user = input.namespace_metadata()?.ino();
            let names = ["mnt", "net", "ipc", "uts", "pid", "pid_for_children"];
            let before =
                names.map(|name| fs::metadata(format!("/proc/self/ns/{name}")).unwrap().ino());
            let (scope, devices) = input.enter()?;
            for (name, old) in names.into_iter().zip(before) {
                let new = fs::metadata(format!("/proc/self/ns/{name}"))?.ino();
                if name == "pid" {
                    assert_eq!(new, old);
                } else {
                    assert_ne!(new, old);
                }
            }
            assert_eq!(fs::metadata("/proc/self/ns/user")?.ino(), expected_user);
            let status = fs::read_to_string("/proc/self/status")?;
            for line in status.lines() {
                let mut values = line.split_whitespace();
                match values.next() {
                    Some("Uid:" | "Gid:") => assert_eq!(values.collect::<Vec<_>>(), ["0"; 4]),
                    Some("Groups:") => assert_eq!(values.count(), 0),
                    _ => {}
                }
            }
            for line in fs::read_to_string("/proc/self/mountinfo")?.lines() {
                let optional = line
                    .split_whitespace()
                    .skip(6)
                    .take_while(|field| *field != "-");
                assert!(
                    !optional
                        .into_iter()
                        .any(|field| field.starts_with("shared:") || field.starts_with("master:"))
                );
            }
            println!("NATIVE_NAMESPACE_IDENTITIES_AND_CREDENTIALS_PASS");
            let mut setup = Command::new(std::env::current_exe()?);
            setup
                .args(["native-root", script])
                .env_clear()
                .stdin(devices.into_input()?);
            if args.get(3).is_some_and(|arg| arg == "--probe-extra-fd") {
                setup.arg("--probe-extra-fd");
            }
            let child = scope.spawn(setup)?;
            let status = child
                .wait_until(Instant::now() + Duration::from_secs(30))?
                .status();
            use std::os::unix::process::ExitStatusExt;
            std::process::exit(status.code().unwrap_or(128 + status.signal().unwrap_or(1)));
        }
        if args.get(1).is_some_and(|a| a == "native-root") {
            let script = args.get(2).ok_or("script required")?;
            let scope = PrivateMountNamespace::enter()?;
            let selected =
                CommandRoot::from_current_directory(scope, CommandDevices::from_input()?)?;
            let filter = CommandSyscalls::compile()?;
            let entered = selected.enter()?;
            let command = entered.restrict(script, CommandPrivileges::filesystem(), filter)?;
            if args.get(3).is_some_and(|arg| arg == "--probe-extra-fd") {
                let extra = fs::File::open("/dev/null")?;
                let error = command.execute().unwrap_err();
                assert!(error.to_string().contains("would remain in command init"));
                drop(extra);
                println!("RESTRICTED_COMMAND_EXTRA_DESCRIPTOR_REJECTED_PASS");
                return Ok(());
            }
            command.execute()?;
            unreachable!();
        }
        if args
            .get(1)
            .is_some_and(|a| a == "privileges" || a == "syscalls")
        {
            let policy = CommandPrivileges::filesystem();
            let expected = format!("{:016x}", policy.allowed().bits());
            let syscalls = CommandSyscalls::compile()?;
            policy.apply()?;
            if args[1] == "syscalls" {
                syscalls.apply()?;
                // Benign owned-resource checks only. libc thread creation must
                // fall back from clone3 while namespace creation is denied.
                assert_eq!(std::thread::spawn(|| 42).join().unwrap(), 42);
                fs::write("/work/filtered-write", "ordinary-io")?;
                assert_eq!(fs::read_to_string("/work/filtered-write")?, "ordinary-io");
                fs::remove_file("/work/filtered-write")?;
                let blocked = Command::new("/usr/bin/unshare")
                    .args(["--user", "/usr/bin/true"])
                    .env_clear()
                    .output()?;
                assert!(!blocked.status.success());
                assert!(
                    String::from_utf8_lossy(&blocked.stderr).contains("Operation not permitted")
                );
                println!("NATIVE_SYSCALL_NAMESPACE_DENIAL_PASS");
            }
            let script = format!(
                r#"
                caps=0
                nnp=0
                while read -r key value rest; do
                    case "$key" in
                        CapEff:|CapPrm:|CapBnd:) test "$value" = {expected}; caps=$((caps+1));;
                        CapInh:|CapAmb:) test "$value" = 0000000000000000; caps=$((caps+1));;
                        NoNewPrivs:) test "$value" = 1; nnp=1;;
                    esac
                done < /proc/self/status
                test "$caps" = 5
                test "$nnp" = 1
                echo NATIVE_PRIVILEGES_EXEC_PASS
            "#
            );
            return Err(Command::new("/usr/bin/bash")
                .args(["-eu", "-c", &script])
                .env_clear()
                .stdin(Stdio::null())
                .exec()
                .into());
        }
        let mode = args.get(2).ok_or("mode required")?;
        if args.get(1).is_some_and(|a| a == "helper") {
            return Self::helper(mode);
        }
        // Fixture init retains the original filesystem mount. The native
        // namespace pin below retains its identity until policy cleanup/poweroff.
        for name in ["orphan-release", "survivor-ready"] {
            match fs::remove_file(Path::new("/work/origin").join(name)) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        let reaper = ChildReaper::install()?;
        let recovery = Path::new("/work/integration-records");
        fs::create_dir(recovery)?;
        fs::set_permissions(recovery, fs::Permissions::from_mode(0o700))?;
        let lease = RecoveryDirectory::open(recovery)?;
        let mut creator = Command::new(std::env::current_exe()?);
        creator.arg("namespace-seed").env_clear();
        let namespace = UserNamespace::<Mapped>::create(
            creator,
            IdRange::new(100000, 65536)?,
            IdRange::new(100000, 65536)?,
            Instant::now() + Duration::from_secs(5),
        )?;
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        let namespace = namespace.pin(&lease)?;
        let filesystem = FilesystemPin::create(&lease, fs::File::open("/work/origin")?)?;
        fs::write(
            "/work/userns-path",
            recovery
                .join(namespace.record().name())
                .as_os_str()
                .as_encoded_bytes(),
        )?;
        println!("NATIVE_MAPPED_NAMESPACE_CREATOR_REAPED_PASS");
        if mode != "policy-off" {
            let policy = PolicyFixture::prepare(&lease, namespace, filesystem)?;
            return Self::protected_step(lease, reaper, policy, mode);
        }
        let policy = PolicyFixture::install(&lease, namespace, filesystem)?;
        println!("NATIVE_MAPPED_NAMESPACE_CREATOR_REAPED_PASS");

        let parent = Path::new("/work/cgroups");
        let domain = RecordedDomain::create(&lease, parent)?;
        let name = domain.record().name().to_owned();
        let mut command = Command::new(std::env::current_exe()?);
        command.args(["helper", mode]).env_clear();
        let helper = domain
            .launch(command, Instant::now() + Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        if mode == "admission-timeout" {
            let failure = helper.admit_until(Instant::now()).unwrap_err();
            let (source, cancellation) = failure.into_parts();
            assert_eq!(source.kind(), std::io::ErrorKind::TimedOut);
            let deadline = Instant::now() + Duration::from_secs(5);
            cancellation
                .retry_until(deadline)
                .map_err(|error| error.to_string())?
                .reap_until(deadline)
                .map_err(|error| error.to_string())?
                .with_reaper(reaper)
                .drain_until(deadline)
                .map_err(|error| error.to_string())?
                .finish(parent)
                .map_err(|error| error.to_string())?;
            policy.disable()?;
            assert!(!Path::new("/work/origin/command-started").exists());
            assert!(
                fs::read_to_string("/proc/thread-self/children")?
                    .trim()
                    .is_empty()
            );
            drop(lease);
            fs::remove_dir(recovery)?;
            println!("ADMISSION_TIMEOUT_CANCELS_WITHOUT_COMMAND_PASS");
            return Ok(());
        }
        let helper = helper
            .admit_until(Instant::now() + Duration::from_secs(5))
            .map_err(|error| error.to_string())?;
        let ready = Path::new("/work/origin/command-started");
        Readiness::new(ready)?.wait_until(Instant::now() + Duration::from_secs(5), || {
            helper.supervision().peer_lost()
        })?;
        if mode == "vm-death" {
            fs::write("/work/supervisor-ready", "running\n")?;
            // The independent guest controller terminates this process. There
            // must be no successful step receipt or in-process cleanup path.
            helper
                .supervision()
                .wait_for_loss_until(Instant::now() + Duration::from_secs(30))?;
            return Err("command ended before supervisor-death exercise".into());
        }
        let execution =
            Instant::now() + Duration::from_millis(if mode == "timeout" { 200 } else { 5000 });
        let observation = helper.supervision().wait_for_loss_until(execution);
        if mode == "timeout" {
            assert_eq!(
                observation.unwrap_err().kind(),
                std::io::ErrorKind::TimedOut
            );
        } else {
            observation?;
            // The fixture helper has closed its channel but cannot exit until
            // explicitly released. Channel loss must not count as process exit.
            if mode != "shell-failure" {
                assert_eq!(
                    helper
                        .supervision()
                        .wait_for_peer_exit_until(Instant::now())
                        .unwrap_err()
                        .kind(),
                    std::io::ErrorKind::TimedOut,
                );
                ExitGate::release()?;
            }
            helper.supervision().wait_for_peer_exit_until(execution)?;
        }
        let teardown = Instant::now() + Duration::from_secs(5);
        let (status, _) = helper
            .cancel_until(teardown)
            .map_err(|e| e.to_string())?
            .reap_until(teardown)
            .map_err(|e| e.to_string())?
            .with_reaper(reaper)
            .drain_until(teardown)
            .map_err(|e| e.to_string())?
            .finish(parent)
            .map_err(|e| e.to_string())?;
        // This is the only policy teardown site: cancellation, helper reaping
        // and descendant draining have all completed above.
        policy.disable()?;
        assert_eq!(status.success(), mode == "success" || mode == "policy-off");
        assert!(!parent.join(name).exists());
        assert_eq!(fs::metadata(ready)?.uid(), 0);
        assert_eq!(fs::read_to_string(ready)?, "image-write\n");
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        assert_eq!(fs::read_to_string("/supervisor-secret")?, "unchanged");
        drop(lease);
        fs::remove_dir(recovery)?;
        println!("INTEGRATED_COMMAND_{}_PASS", mode.to_uppercase());
        Ok(())
    }
    fn protected_step(
        lease: RecoveryDirectory,
        reaper: ChildReaper,
        policy: LabelPolicy<BoundDisabled>,
        mode: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let recovery = Path::new("/work/integration-records");
        let parent = Path::new("/work/cgroups");
        let domain = RecordedDomain::create(&lease, parent)?;
        let name = domain.record().name().to_owned();
        let mut command = Command::new(std::env::current_exe()?);
        command.args(["helper", mode]).env_clear();
        let helper = domain
            .launch(command, Instant::now() + Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        let step = conglobate::step::Step::<conglobate::step::Ready>::new(policy, helper, reaper)
            .map_err(|error| error.to_string())?;
        println!("PRODUCTION_STEP_OWNS_POLICY_AND_COMMAND_PASS");
        if mode == "pre-admission-cancel" {
            let deadline = Instant::now() + Duration::from_secs(5);
            let (_, _, policy, ()) = step
                .cancel_until(deadline)
                .map_err(|e| e.to_string())?
                .reap_until(deadline)
                .map_err(|e| e.to_string())?
                .drain_until(deadline)
                .map_err(|e| e.to_string())?
                .finish_record(parent)
                .map_err(|e| e.to_string())?
                .disable()
                .map_err(|e| e.to_string())?
                .into_parts();
            PolicyFixture::cleanup(policy)?;
            assert!(!Path::new("/work/origin/command-started").exists());
            assert!(
                fs::read_to_string("/proc/thread-self/children")?
                    .trim()
                    .is_empty()
            );
            drop(lease);
            fs::remove_dir(recovery)?;
            println!("PRODUCTION_STEP_CANCEL_BEFORE_ADMISSION_PASS");
            return Ok(());
        }

        if mode == "admission-timeout" {
            let failure = step.admit_until(Instant::now()).unwrap_err();
            let conglobate::step::StartFailure::Admission(failure) = failure else {
                return Err(failure.to_string().into());
            };
            let (source, cancellation) = failure.into_parts();
            assert_eq!(source.kind(), std::io::ErrorKind::TimedOut);
            let deadline = Instant::now() + Duration::from_secs(5);
            let (_, _, policy, ()) = cancellation
                .cancel_until(deadline)
                .map_err(|error| error.to_string())?
                .reap_until(deadline)
                .map_err(|error| error.to_string())?
                .drain_until(deadline)
                .map_err(|error| error.to_string())?
                .finish_record(parent)
                .map_err(|error| error.to_string())?
                .disable()
                .map_err(|error| error.to_string())?
                .into_parts();
            PolicyFixture::cleanup(policy)?;
            assert!(!Path::new("/work/origin/command-started").exists());
            assert!(
                fs::read_to_string("/proc/thread-self/children")?
                    .trim()
                    .is_empty()
            );
            drop(lease);
            fs::remove_dir(recovery)?;
            println!("ADMISSION_TIMEOUT_CANCELS_WITHOUT_COMMAND_PASS");
            return Ok(());
        }
        let helper = step
            .admit_until(Instant::now() + Duration::from_secs(5))
            .map_err(|error| error.to_string())?;
        let ready = Path::new("/work/origin/command-started");
        Readiness::new(ready)?.wait_until(Instant::now() + Duration::from_secs(5), || {
            helper.supervision().peer_lost()
        })?;
        if mode == "vm-death" {
            fs::write("/work/supervisor-ready", "running\n")?;
            // The independent guest controller terminates this process. There
            // must be no successful step receipt or in-process cleanup path.
            helper
                .supervision()
                .wait_for_loss_until(Instant::now() + Duration::from_secs(30))?;
            return Err("command ended before supervisor-death exercise".into());
        }
        let execution =
            Instant::now() + Duration::from_millis(if mode == "timeout" { 200 } else { 5000 });
        let observation = helper.supervision().wait_for_loss_until(execution);
        if mode == "timeout" {
            assert_eq!(
                observation.unwrap_err().kind(),
                std::io::ErrorKind::TimedOut
            );
        } else {
            observation?;
            // The fixture helper has closed its channel but cannot exit until
            // explicitly released. Channel loss must not count as process exit.
            if mode != "shell-failure" {
                assert_eq!(
                    helper
                        .supervision()
                        .wait_for_peer_exit_until(Instant::now())
                        .unwrap_err()
                        .kind(),
                    std::io::ErrorKind::TimedOut,
                );
                ExitGate::release()?;
            }
            helper.supervision().wait_for_peer_exit_until(execution)?;
        }
        let teardown = Instant::now() + Duration::from_secs(5);
        let (status, _, policy, ()) = helper
            .cancel_until(teardown)
            .map_err(|e| e.to_string())?
            .reap_until(teardown)
            .map_err(|e| e.to_string())?
            .drain_until(teardown)
            .map_err(|e| e.to_string())?
            .finish_record(parent)
            .map_err(|e| e.to_string())?
            .disable()
            .map_err(|error| error.to_string())?
            .into_parts();
        // This is the only policy teardown site: cancellation, helper reaping
        // and descendant draining have all completed above.
        PolicyFixture::cleanup(policy)?;
        assert_eq!(status.success(), mode == "success" || mode == "policy-off");
        assert!(!parent.join(name).exists());
        assert_eq!(fs::metadata(ready)?.uid(), 0);
        assert_eq!(fs::read_to_string(ready)?, "image-write\n");
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        assert_eq!(fs::read_to_string("/supervisor-secret")?, "unchanged");
        drop(lease);
        fs::remove_dir(recovery)?;
        println!("INTEGRATED_COMMAND_{}_PASS", mode.to_uppercase());
        Ok(())
    }
    fn root_launch_probe<'ns>(
        mount: Mount<'ns, conglobate::filesystem::Attached>,
        cancel: bool,
    ) -> Result<Mount<'ns, conglobate::filesystem::Attached>, Box<dyn std::error::Error>> {
        use std::os::{fd::OwnedFd, unix::net::UnixDatagram};
        let expected = mount.root_metadata()?;
        let failure = mount
            .launch(Command::new("/nonexistent-root-launch-test"))
            .unwrap_err();
        let (error, mount) = failure.into_parts();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        let (control, child) = UnixDatagram::pair()?;
        control.set_read_timeout(Some(Duration::from_secs(5)))?;
        let mut setup = Command::new(std::env::current_exe()?);
        setup
            .arg("root-launch-probe")
            .current_dir("/")
            .stdin(Stdio::from(OwnedFd::from(child)));
        let running = mount.launch(setup).map_err(|error| error.to_string())?;
        let mut ready = [0; 6];
        let count = control.recv(&mut ready)?;
        assert_eq!(&ready[..count], b"ready");
        let (error, running) = running.wait_until(Instant::now()).unwrap_err().into_parts();
        assert!(matches!(error, conglobate::process::WaitError::Deadline));
        let deadline = Instant::now() + Duration::from_secs(5);
        let (mount, status) = if cancel {
            running.terminate_until(deadline)
        } else {
            assert_eq!(control.send(b"exit")?, 4);
            running.wait_until(deadline)
        }
        .map_err(|error| error.to_string())?;
        if cancel {
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(status.signal(), Some(9));
            println!("SCOPED_ROOT_TERMINATION_PASS");
        } else {
            assert!(status.success());
        }
        let retained = mount.root_metadata()?;
        assert_eq!(
            (retained.dev(), retained.ino()),
            (expected.dev(), expected.ino())
        );
        println!("SCOPED_ROOT_LAUNCH_FAILURE_AND_RETRY_PASS");
        Ok(mount)
    }

    fn command_script(mode: &str) -> String {
        let checks = r#"
            test "$PPID" = 1
            test "$$" -ne 1
            for interface in exe mem maps environ fd/0; do
                if (: < "/proc/1/$interface") 2>/dev/null; then exit 1; fi
            done
            # The intermediate shell exits, leaving a live orphan that waits
            # for a file-based release. No timing delay determines adoption.
            /bin/sh -c '/bin/sh -c "while test ! -e /orphan-release; do :; done" & echo $! > /orphan-id'
            read -r orphan < /orphan-id
            adopted=0
            while read -r key value rest; do
                if test "$key" = PPid:; then test "$value" = 1; adopted=1; fi
            done < "/proc/$orphan/status"
            test "$adopted" = 1
            printf 'exit' > /orphan-release
            while test -e "/proc/$orphan"; do :; done
            echo NATIVE_NAMESPACE_INIT_ADOPTION_PASS
            test ! -e /supervisor-secret
            test ! -e /work
            test ! -e /idmap-probe
            test ! -e /image-root
            test ! -e /bin/setpriv
            for node in null zero full random urandom; do
                test -c "/dev/$node"
                test -O "/dev/$node"
            done
            printf 'discarded' > /dev/null
            if (printf 'full' > /dev/full) 2>/dev/null; then exit 1; fi
            test ! -e /dev/fuse
            set -- /dev/*
            test "$#" = 13
            test -c /dev/tty
            if (: > /dev/tty) 2>/dev/null; then exit 1; fi
            test -c /dev/ptmx
            test ! -e /dev/pts/0
            exec 3<> /dev/ptmx
            test -c /dev/pts/0
            exec 3>&-
            test ! -e /dev/pts/0
            test ! -e /dev/shm/step-data
            printf 'temporary' > /dev/shm/step-data
            read -r -N 9 shm < /dev/shm/step-data
            test "$shm" = temporary
            if (printf no > /sys/must-be-read-only) 2>/dev/null; then exit 1; fi
            restricted=0
            while read -r id parent device root point options rest; do
                case "$point" in
                    /proc/sys|/proc/sysrq-trigger|/proc/irq|/proc/bus)
                        case ",$options," in
                            *,ro,*) restricted=$((restricted + 1));;
                            *) exit 1;;
                        esac;;
                esac
            done < /proc/self/mountinfo
            test "$restricted" = 4
            # Open-only denial: do not send a command to any kernel control.
            if (: > /proc/sysrq-trigger) 2>/dev/null; then exit 1; fi
            caps=0
            nnp=0
            while read -r key value rest; do
                case "$key" in
                    CapPrm:|CapEff:|CapBnd:)
                        test "$value" = EXPECTED_CAPABILITIES
                        caps=$((caps + 1));;
                    CapInh:|CapAmb:)
                        test "$value" = 0000000000000000
                        caps=$((caps + 1));;
                    NoNewPrivs:) test "$value" = 1; nnp=1;;
                esac
            done < /proc/self/status
            test "$caps" = 5
            test "$nnp" = 1
            read -r groups_policy < /proc/self/setgroups
            test "$groups_policy" = allow
            read -r inside outside count < /proc/self/uid_map
            test "$inside" = 0
            test "$outside" = 100000
            test "$count" = 65536
            printf 'privileges-checked\n' > /command-privileges
            printf 'image-write\n' > /command-started
        "#;
        let checks = checks.replace(
            "EXPECTED_CAPABILITIES",
            &format!("{:016x}", CommandPrivileges::filesystem().allowed().bits()),
        );
        let labels = if mode == "policy-off" {
            r#"
                printf 'owned' > /label-policy-off
                if setfattr -n security.selinux -v 'system_u:object_r:etc_t:s0' /label-policy-off; then exit 1; fi
            "#
        } else {
            r#"
                printf 'owned' > /labelled
                setfattr -n security.selinux -v 'system_u:object_r:etc_t:s0' /labelled
                test "$(getfattr --only-values -n security.selinux /labelled)" = 'system_u:object_r:etc_t:s0'
                setfattr -x security.selinux /labelled
                if getfattr --only-values -n security.selinux /labelled; then exit 1; fi
                setfattr -n security.selinux -v 'system_u:object_r:etc_t:s0' /labelled
                printf 'owned' > /dev/shm/outside-policy
                if setfattr -n security.selinux -v 'system_u:object_r:etc_t:s0' /dev/shm/outside-policy; then exit 1; fi
                if setfattr -n security.selinux.extra -v 'test' /labelled; then exit 1; fi
            "#
        };
        let script = format!(
            "{labels}\n{checks}\n{}",
            if mode == "vm-death" {
                r#"
                    while test ! -e /supervisor-gone; do :; done
                    setfattr -n security.selinux -v 'system_u:object_r:var_t:s0' /labelled
                    test "$(getfattr --only-values -n security.selinux /labelled)" = 'system_u:object_r:var_t:s0'
                    if setfattr -n security.selinux -v 'system_u:object_r:var_t:s0' /dev/shm/outside-policy; then exit 1; fi
                    printf 'checked' > /death-policy-checked.tmp
                    mv /death-policy-checked.tmp /death-policy-checked
                    while :; do :; done
                "#
            } else if mode == "timeout" {
                "while :; do :; done"
            } else if mode == "shell-failure" {
                "exit 37"
            } else {
                r#"
                    /bin/sh -c "printf 'running' > /survivor-ready; while :; do :; done" &
                    while test ! -e /survivor-ready; do :; done
                    printf 'completed\n' > /command-completed
                "#
            }
        );
        script
    }

    fn helper(mode: &str) -> Result<(), Box<dyn std::error::Error>> {
        let channel = Supervision::from_stdin()?
            .exchange_until(Instant::now() + Duration::from_secs(5))?
            .join_domain_until(Instant::now() + Duration::from_secs(5))?
            .await_admission_until(Instant::now() + Duration::from_secs(5))?;
        let namespace = PrivateMountNamespace::enter()?;
        let userns = fs::File::open(fs::read_to_string("/work/userns-path")?)?;
        let mount = Mount::<Detached>::clone_from(&fs::File::open("/work/origin")?, &namespace)?
            .idmap(userns.try_clone()?)
            .map_err(|e| e.to_string())?
            .attach(MountPoint::create(Path::new("/work"))?)
            .map_err(|e| e.to_string())?;
        let mount = Self::root_launch_probe(mount, false)?;
        let mount = Self::root_launch_probe(mount, true)?;
        let script = Self::command_script(mode);
        let mut setup = Command::new(std::env::current_exe()?);
        setup.arg("native-namespaces").arg(&script).stdin(
            CommandSetup::new(userns, CommandDevices::create(100000, 100000)?)?.into_input()?,
        );
        let running = mount.launch(setup).map_err(|e| e.to_string())?;
        let (mount, status) = running
            .wait_until(Instant::now() + Duration::from_secs(30))
            .map_err(|e| e.to_string())?;
        if mode == "shell-failure" {
            assert_eq!(status.code(), Some(37));
            println!("NATIVE_NAMESPACE_INIT_EXIT_STATUS_PASS");
        }
        if !status.success() {
            return Err(format!("command failed: {status}").into());
        }
        mount.close().map_err(|e| e.to_string())?;
        namespace.restore()?;
        let exit = ExitGate::bind()?;
        drop(channel);
        exit.await_release()?;
        Ok(())
    }
}
/// Fixture-only handshake keeps the helper alive after channel closure without
/// relying on scheduler timing. Commands cannot access this supervisor path.
#[derive(Debug)]
struct ExitGate(std::os::unix::net::UnixDatagram);
impl ExitGate {
    const PATH: &str = "/work/helper-exit-release";

    fn bind() -> std::io::Result<Self> {
        let socket = std::os::unix::net::UnixDatagram::bind(Self::PATH)?;
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        Ok(Self(socket))
    }
    fn release() -> std::io::Result<()> {
        let socket = std::os::unix::net::UnixDatagram::unbound()?;
        socket.set_write_timeout(Some(Duration::from_secs(5)))?;
        assert_eq!(socket.send_to(b"exit", Self::PATH)?, 4);
        Ok(())
    }
    fn await_release(self) -> std::io::Result<()> {
        let mut message = [0; 5];
        let count = self.0.recv(&mut message)?;
        assert_eq!(&message[..count], b"exit");
        fs::remove_file(Self::PATH)?;
        Ok(())
    }
}

/// Disabled-policy baseline and post-quiescence cleanup fault injection.
/// Enabled commands use the production Step; this fixture cannot enable maps.
#[derive(Debug)]
struct PolicyFixture {
    policy: LabelPolicy<BoundDisabled>,
}
impl PolicyFixture {
    fn prepare(
        lease: &RecoveryDirectory,
        namespace: UserNamespace<PinnedNamespace>,
        filesystem: FilesystemPin,
    ) -> Result<LabelPolicy<BoundDisabled>, Box<dyn std::error::Error>> {
        let target = FilesystemTarget::new(
            fs::metadata("/proc/self/ns/user")?.ino(),
            filesystem.metadata()?.dev(),
        )?;
        let scope = namespace.label_scope([target])?;
        let pinned = LabelPolicy::new(scope)?
            .load()?
            .pin_disabled(PinDirectory::create(Path::new("/work/pins"))?)?;
        let failure = pinned.bind(namespace, vec![]).unwrap_err();
        let (source, pinned, identities) = failure.into_parts();
        assert!(
            source
                .to_string()
                .contains("one pin per distinct assigned filesystem")
        );
        let (namespace, empty) = identities.into_parts();
        assert!(empty.is_empty());
        let wrong = FilesystemPin::create(lease, fs::File::open("/work")?)?;
        let failure = pinned.bind(namespace, vec![wrong]).unwrap_err();
        let (source, pinned, identities) = failure.into_parts();
        assert!(source.to_string().contains("targets differ"));
        let (namespace, wrong) = identities.into_parts();
        for pin in wrong {
            pin.release()?;
        }
        let pinned = pinned.bind(namespace, vec![filesystem])?;
        println!("BOUND_POLICY_MISMATCH_RETAINS_OWNERS_PASS");
        Ok(pinned)
    }
    fn install(
        lease: &RecoveryDirectory,
        namespace: UserNamespace<PinnedNamespace>,
        filesystem: FilesystemPin,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            policy: Self::prepare(lease, namespace, filesystem)?,
        })
    }
    fn disable(self) -> Result<(), Box<dyn std::error::Error>> {
        Self::cleanup(self.policy)
    }
    fn cleanup(policy: LabelPolicy<BoundDisabled>) -> Result<(), Box<dyn std::error::Error>> {
        let paths = fs::read_dir("/work/pins")?.collect::<Result<Vec<_>, _>>()?;
        assert_eq!(paths.len(), 1);
        let blocker = paths[0].path().join("cleanup-test");
        fs::create_dir(&blocker)?;
        let failure = policy.close().unwrap_err();
        let (source, cleanup) = failure.into_parts();
        assert_eq!(source.kind(), std::io::ErrorKind::DirectoryNotEmpty);
        assert_eq!(fs::read_dir("/work/integration-records")?.count(), 2);
        fs::remove_dir(blocker)?;
        let identities = cleanup.retry()?;
        println!("BOUND_POLICY_CLEANUP_RETRY_RETAINS_IDENTITIES_PASS");
        let (namespace, filesystems) = identities.into_parts();
        for filesystem in filesystems {
            filesystem.release()?;
        }
        namespace.release()?;
        assert_eq!(fs::read_dir("/work/pins")?.count(), 0);
        Ok(())
    }
}

fn main() {
    if let Err(error) = Integration::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
