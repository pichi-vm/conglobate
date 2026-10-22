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
            entered.execute(script, CommandPrivileges::filesystem(), filter)?;
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
            let (_, _, policy) = step
                .cancel_until(deadline)
                .map_err(|e| e.to_string())?
                .reap_until(deadline)
                .map_err(|e| e.to_string())?
                .drain_until(deadline)
                .map_err(|e| e.to_string())?
                .finish_record(parent)
                .map_err(|e| e.to_string())?
                .disable()
                .map_err(|e| e.to_string())?;
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
            let (_, _, policy) = cancellation
                .cancel_until(deadline)
                .map_err(|error| error.to_string())?
                .reap_until(deadline)
                .map_err(|error| error.to_string())?
                .drain_until(deadline)
                .map_err(|error| error.to_string())?
                .finish_record(parent)
                .map_err(|error| error.to_string())?
                .disable()
                .map_err(|error| error.to_string())?;
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
        let (status, _, policy) = helper
            .cancel_until(teardown)
            .map_err(|e| e.to_string())?
            .reap_until(teardown)
            .map_err(|e| e.to_string())?
            .drain_until(teardown)
            .map_err(|e| e.to_string())?
            .finish_record(parent)
            .map_err(|e| e.to_string())?
            .disable()
            .map_err(|error| error.to_string())?;
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
                    printf 'checked' > /death-policy-checked
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
