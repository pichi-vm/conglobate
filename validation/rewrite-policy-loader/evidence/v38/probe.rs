// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]
use conglobate::policy::{FilesystemTarget, LabelPolicy, LabelScope, PinDirectory};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
};

#[derive(Debug)]
struct Probe;
impl Probe {
    fn pin_lifetime_child() -> Result<(), Box<dyn std::error::Error>> {
        let holder = fs::read_to_string("/work/userns-pid")?;
        let namespace = fs::metadata(format!("/proc/{}/ns/user", holder.trim()))?.ino();
        let target = FilesystemTarget::new(
            fs::metadata("/proc/self/ns/user")?.ino(),
            fs::metadata("/work/origin")?.dev(),
        )?;
        let scope = LabelScope::new(namespace, 100000, [target])?;
        let _policy = LabelPolicy::new(scope)?
            .load()?
            .pin_disabled(PinDirectory::create(std::path::Path::new(
                "/work/pins/lifetime",
            ))?)?;
        fs::write("/work/pins-ready", b"ready")?;
        std::thread::sleep(std::time::Duration::from_secs(30));
        Err("fixture parent failed to terminate loader".into())
    }

    fn pin_lifetime_test() -> Result<(), Box<dyn std::error::Error>> {
        use libbpf_rs::{Link, MapCore, MapFlags, MapHandle};
        use std::os::unix::process::ExitStatusExt;
        let parent = std::path::Path::new("/work/pins/lifetime");
        fs::create_dir(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        let mut child = std::process::Command::new(std::env::current_exe()?)
            .arg("--pin-lifetime-child")
            .spawn()?;
        Self::wait_for_file(
            std::path::Path::new("/work/pins-ready"),
            std::time::Instant::now() + std::time::Duration::from_secs(5),
        )?;
        let paths = fs::read_dir(parent)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(paths.len(), 1);
        let directory = &paths[0];
        let names = ["check_set", "check_remove", "skip_label_cap"];
        let mut identities = Vec::new();
        for name in names {
            let info = Link::open(directory.join(name))?.info()?;
            identities.push((info.id, info.prog_id));
        }
        // All witness descriptors are closed before SIGKILL. The VM's mount
        // namespace (held by init) owns the bpffs pins; no guardian is running.
        child.kill()?;
        assert_eq!(child.wait()?.signal(), Some(9));
        for (name, identity) in names.into_iter().zip(identities) {
            let info = Link::open(directory.join(name))?.info()?;
            assert_eq!((info.id, info.prog_id), identity);
            assert_ne!(info.prog_id, 0);
        }
        let config = MapHandle::from_pinned_path(directory.join("config"))?;
        assert_eq!(
            config.lookup(&0u32.to_ne_bytes(), MapFlags::ANY)?,
            Some(vec![0; 8])
        );
        let targets = MapHandle::from_pinned_path(directory.join("targets"))?;
        assert_eq!(targets.keys().count(), 1);
        drop((config, targets));
        // Only disabled policy is tested here. No command has been admitted.
        for name in [
            "skip_label_cap",
            "check_remove",
            "check_set",
            "targets",
            "config",
        ] {
            fs::remove_file(directory.join(name))?;
        }
        fs::remove_dir(directory)?;
        fs::remove_dir(parent)?;
        println!("PRODUCTION_POLICY_PINS_SURVIVE_LOADER_KILL_PASS");
        Ok(())
    }

    fn run() -> Result<(), Box<dyn std::error::Error>> {
        if !std::path::Path::new("/conglobate-disposable-validation-vm").exists() {
            return Err("disposable VM marker required".into());
        }
        let arguments = std::env::args_os().collect::<Vec<_>>();
        if arguments
            .get(1)
            .is_some_and(|a| a == "--pin-lifetime-child")
        {
            return Self::pin_lifetime_child();
        }
        if arguments.get(1).is_some_and(|a| a == "--pin-lifetime-test") {
            return Self::pin_lifetime_test();
        }

        if arguments
            .get(1)
            .is_some_and(|a| a == "--stall-registration")
        {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            let _channel =
                conglobate::process::Supervision::from_stdin()?.exchange_until(deadline)?;
            std::thread::sleep(std::time::Duration::from_secs(30));
            return Ok(());
        }

        if arguments
            .get(1)
            .is_some_and(|a| a == "--independent-guardian")
        {
            return Self::independent_guardian(std::path::Path::new(
                arguments.get(2).ok_or("missing recovery path")?,
            ));
        }
        if arguments
            .get(1)
            .is_some_and(|a| a == "--independent-guardian-test")
        {
            return Self::independent_guardian_test();
        }
        if arguments
            .get(1)
            .is_some_and(|a| a == "--departing-supervisor")
        {
            let root = std::path::Path::new(arguments.get(2).ok_or("missing recovery path")?);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            let mut command = std::process::Command::new(std::env::current_exe()?);
            command.arg("--independent-guardian").arg(root);
            let (_guardian, _channel) =
                conglobate::process::Process::launch_supervised(command, deadline)?;
            Self::wait_for_file(&root.join("ready"), deadline)?;
            // Exit without Rust destructors: no owner kills the guardian.
            std::process::exit(0);
        }

        if arguments.get(1).is_some_and(|a| a == "--orphan-helper") {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            let _watch = conglobate::process::Supervision::from_stdin()?
                .exchange_until(deadline)?
                .join_domain_until(deadline)?;
            let _child = std::process::Command::new("/usr/bin/sleep")
                .arg("30")
                .spawn()?;
            // std::process::Child does not terminate on Drop. This benign orphan
            // remains contained for the guardian's cancellation/reaping test.
            return Ok(());
        }
        if arguments
            .get(1)
            .is_some_and(|a| a == "--guardian-drain-test")
        {
            return Self::guardian_drain();
        }

        if arguments.get(1).is_some_and(|a| a == "--reaper-test") {
            let child: conglobate::process::Process<conglobate::process::Running> =
                std::process::Command::new("/usr/bin/sleep")
                    .arg("30")
                    .spawn()?
                    .into();
            assert!(conglobate::process::ChildReaper::install().is_err());
            child.terminate_until(std::time::Instant::now() + std::time::Duration::from_secs(5))?;
            let reaper = conglobate::process::ChildReaper::install()?;
            assert!(conglobate::process::ChildReaper::install().is_err());
            let mut parent = std::process::Command::new("/bin/bash")
                .args(["-c", "sleep 1 & exit 0"])
                .env_clear()
                .env("PATH", "/usr/bin")
                .spawn()?;
            assert!(parent.wait()?.success());
            let (source, reaper) = reaper
                .drain_until(std::time::Instant::now())
                .unwrap_err()
                .into_parts();
            assert_eq!(source.kind(), std::io::ErrorKind::TimedOut);
            let drained = reaper
                .drain_until(std::time::Instant::now() + std::time::Duration::from_secs(5))?;
            assert_eq!(drained.collected(), 1);
            assert!(
                fs::read_to_string("/proc/thread-self/children")?
                    .trim()
                    .is_empty()
            );
            println!("PRODUCTION_CHILD_SUBREAPER_PASS");
            return Ok(());
        }

        if arguments
            .get(1)
            .is_some_and(|argument| argument == "--guardian-peer")
        {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            let watch = conglobate::process::Supervision::from_stdin()?.exchange_until(deadline)?;
            watch.wait_for_loss_until(deadline)?;
            return Ok(());
        }

        if arguments
            .get(1)
            .is_some_and(|argument| argument == "--publish-domain")
        {
            let recovery = arguments.get(2).ok_or("missing recovery path")?;
            let parent = arguments.get(3).ok_or("missing domain parent")?;
            let lease =
                conglobate::anchor::RecoveryDirectory::open(std::path::Path::new(recovery))?;
            let domain =
                conglobate::domain::CommandDomain::create(&lease, std::path::Path::new(parent))?;
            assert!(domain.registration().is_err());
            let record = lease.save_domain(&domain)?;
            println!("{}", record.name());
            std::io::Write::flush(&mut std::io::stdout())?;
            std::process::exit(0);
        }

        if arguments
            .get(1)
            .is_some_and(|argument| argument == "--registration-close")
        {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            let _watch =
                conglobate::process::Supervision::from_stdin()?.exchange_until(deadline)?;
            return Ok(());
        }
        if arguments
            .get(1)
            .is_some_and(|argument| argument == "--domain-helper")
        {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            let watch = conglobate::process::Supervision::from_stdin()?
                .exchange_until(deadline)?
                .join_domain_until(deadline)?;
            for entry in fs::read_dir("/proc/self/fd")? {
                if let Ok(path) = fs::read_link(entry?.path()) {
                    assert!(path.file_name().is_none_or(|name| name != "cgroup.procs"));
                }
            }
            let child: conglobate::process::Process<conglobate::process::Running> =
                std::process::Command::new("/usr/bin/sleep")
                    .arg("30")
                    .env_clear()
                    .stdin(std::process::Stdio::null())
                    .spawn()?
                    .into();
            watch.wait_for_loss_until(deadline)?;
            child.terminate_until(deadline)?;
            return Ok(());
        }

        if arguments
            .get(1)
            .is_some_and(|argument| argument == "--check-lease")
        {
            let path = arguments.get(2).ok_or("missing lease path")?;
            drop(conglobate::anchor::RecoveryDirectory::open(
                std::path::Path::new(path),
            )?);
            return Ok(());
        }
        if arguments
            .get(1)
            .is_some_and(|argument| argument == "--publish-pin")
        {
            let path = arguments.get(2).ok_or("missing lease path")?;
            let lease = conglobate::anchor::RecoveryDirectory::open(std::path::Path::new(path))?;
            let pin = conglobate::anchor::NamespacePin::create(
                &lease,
                fs::File::open("/proc/self/ns/user")?,
            )?;
            lease.save_namespace(pin.record())?;
            println!("{}", pin.record().name().to_str().unwrap());
            std::io::Write::flush(&mut std::io::stdout())?;
            // Deliberately bypass all Rust destructors; only kernel exit cleanup
            // closes the lease and namespace descriptors. Guest fixture only.
            std::process::exit(0);
        }
        Self::command_domain()?;
        Self::recorded_domain()?;
        Self::contained_helper()?;
        Self::registered_guardian()?;
        Self::watched_startup()?;
        Self::owned_watched_startup()?;
        assert!(
            std::process::Command::new(std::env::current_exe()?)
                .arg("--independent-guardian-test")
                .status()?
                .success()
        );
        assert!(
            std::process::Command::new(std::env::current_exe()?)
                .arg("--guardian-drain-test")
                .status()?
                .success()
        );
        assert!(
            std::process::Command::new(std::env::current_exe()?)
                .arg("--reaper-test")
                .status()?
                .success()
        );
        let holder = fs::read_to_string("/work/userns-pid")?;
        let namespace = fs::metadata(format!("/proc/{}/ns/user", holder.trim()))?.ino();
        let owner = fs::metadata("/proc/self/ns/user")?.ino();
        let device = fs::metadata("/work/origin")?.dev();
        let target = FilesystemTarget::new(owner, device)?;
        let before = fs::read_dir("/proc/self/fd")?.count();
        for _ in 0..3 {
            let scope = LabelScope::new(namespace, 100000, [target])?;
            let loaded = LabelPolicy::new(scope)?.load()?;
            assert_eq!(loaded.scope().targets().count(), 1);
            // No hooks are attached. Dropping must release all kernel FDs.
            drop(loaded);
            assert_eq!(fs::read_dir("/proc/self/fd")?.count(), before);
        }
        println!("PRODUCTION_DISABLED_POLICY_LOAD_DROP_PASS");
        {
            let _restricted = EffectiveCapabilities::clear()?;
            let scope = LabelScope::new(namespace, 100000, [target])?;
            let error = LabelPolicy::new(scope)?.load().unwrap_err();
            assert!(matches!(error, conglobate::policy::PolicyError::Object(_)));
            assert_eq!(fs::read_dir("/proc/self/fd")?.count(), before);
            println!("PRODUCTION_POLICY_PERMISSION_FAILURE_PASS: {error}");
        }
        let scope = LabelScope::new(namespace, 100000, [target])?;
        drop(LabelPolicy::new(scope)?.load()?);
        assert_eq!(fs::read_dir("/proc/self/fd")?.count(), before);
        println!("PRODUCTION_POLICY_RELOAD_AFTER_FAILURE_PASS");
        let parent = std::path::Path::new("/work/pins/parent");
        fs::create_dir(parent)?;
        for explicit in [true, false] {
            let scope = LabelScope::new(namespace, 100000, [target])?;
            let loaded = LabelPolicy::new(scope)?.load()?;
            let pinned = loaded.pin_disabled(PinDirectory::create(parent)?)?;
            let directories = fs::read_dir(parent)?.collect::<Result<Vec<_>, _>>()?;
            assert_eq!(directories.len(), 1);
            assert_eq!(fs::metadata(directories[0].path())?.mode() & 0o777, 0o700);
            let mut entries = fs::read_dir(directories[0].path())?
                .map(|entry| entry.map(|entry| entry.file_name()))
                .collect::<Result<Vec<_>, _>>()?;
            entries.sort();
            assert_eq!(
                entries,
                [
                    "check_remove",
                    "check_set",
                    "config",
                    "skip_label_cap",
                    "targets"
                ]
            );
            // DirEntry retains its read_dir descriptor on Linux. Drop the
            // witness before measuring the production owner's cleanup.
            drop(directories);
            if explicit {
                pinned.close()?;
            } else {
                drop(pinned);
            }
            assert_eq!(fs::read_dir(parent)?.count(), 0);
            assert_eq!(fs::read_dir("/proc/self/fd")?.count(), before);
        }
        println!("PRODUCTION_DISABLED_PIN_CLOSE_DROP_PASS");
        let directory = PinDirectory::create(parent)?;
        let location = fs::read_dir(parent)?.next().unwrap()?.path();
        fs::create_dir(location.join("check_remove"))?;
        let scope = LabelScope::new(namespace, 100000, [target])?;
        let failure = LabelPolicy::new(scope)?
            .load()?
            .pin_disabled(directory)
            .unwrap_err();
        assert!(location.join("check_set").exists());
        assert!(location.join("config").exists());
        assert!(location.join("targets").exists());
        let (_, recovery) = failure.into_parts();
        let failure = recovery.cleanup().unwrap_err();
        let (_, recovery) = failure.into_parts();
        fs::remove_dir(location.join("check_remove"))?;
        recovery.cleanup()?;
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        assert_eq!(fs::read_dir("/proc/self/fd")?.count(), before);
        fs::remove_dir(parent)?;
        println!("PRODUCTION_DISABLED_PIN_RECOVERY_PASS");
        let parent = std::path::Path::new("/work/namespace-pins");
        fs::create_dir(parent)?;
        assert!(conglobate::anchor::RecoveryDirectory::open(parent).is_err());
        fs::set_permissions(
            parent,
            <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o700),
        )?;
        let lease = conglobate::anchor::RecoveryDirectory::open(parent)?;
        assert!(!Self::check_lease_in_child(parent)?);
        assert_eq!(
            conglobate::anchor::RecoveryDirectory::open(parent)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::WouldBlock
        );
        assert!(matches!(
            conglobate::anchor::NamespacePin::create(&lease, fs::File::open("/dev/null")?),
            Err(conglobate::anchor::CreateFailure::Before(_))
        ));
        let holder_pid = rustix::process::Pid::from_raw(holder.trim().parse()?).unwrap();
        let holder_identity =
            rustix::process::pidfd_open(holder_pid, rustix::process::PidfdFlags::empty())?;
        let source = fs::File::open(format!("/proc/{}/ns/user", holder.trim()))?;
        let restricted = EffectiveCapabilities::without_sys_admin()?;
        let failure = conglobate::anchor::NamespacePin::create(&lease, source).unwrap_err();
        let conglobate::anchor::CreateFailure::Mount { target, .. } = failure else {
            panic!("expected mount failure with retained target");
        };
        drop(restricted);
        drop(lease);
        assert_eq!(
            conglobate::anchor::RecoveryDirectory::open(parent)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::WouldBlock
        );
        target.cleanup()?;
        let lease = conglobate::anchor::RecoveryDirectory::open(parent)?;
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        let source = fs::File::open(format!("/proc/{}/ns/user", holder.trim()))?;
        let pin = conglobate::anchor::NamespacePin::create(&lease, source)?;
        let record = pin.record().clone();
        lease.save_namespace(&record)?;
        assert_eq!(
            lease.save_namespace(&record).unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        let name = record.name().to_str().unwrap();
        assert_eq!(lease.load_namespace(name)?, record);
        assert!(lease.remove_namespace_record(&record).is_err());
        let private = conglobate::isolation::PrivateMountNamespace::enter()?;
        assert!(conglobate::anchor::RecoveryDirectory::open(parent).is_err());
        assert!(lease.load_namespace(name).is_err());
        assert!(matches!(
            conglobate::anchor::NamespacePin::create(&lease, fs::File::open("/proc/self/ns/user")?),
            Err(conglobate::anchor::CreateFailure::Before(_))
        ));
        let failure = pin.release().unwrap_err();
        let conglobate::anchor::ReleaseFailure::Mounted { pin, .. } = failure else {
            panic!("wrong mount namespace must retain original pin ownership");
        };
        private.restore()?;
        assert_eq!(lease.load_namespace(name)?, record);
        println!("PRODUCTION_RECOVERY_NAMESPACE_GUARD_PASS");
        let record_path = parent.join(format!("{name}.cbor"));
        fs::write(&record_path, [0x84])?;
        assert!(lease.load_namespace(name).is_err());
        fs::write(&record_path, [0u8; 257])?;
        assert!(lease.load_namespace(name).is_err());
        fs::remove_file(&record_path)?;
        std::os::unix::fs::symlink("/dev/null", &record_path)?;
        assert!(lease.load_namespace(name).is_err());
        fs::remove_file(&record_path)?;
        fs::write(&record_path, record.to_cbor())?;
        fs::set_permissions(&record_path, fs::Permissions::from_mode(0o644))?;
        assert!(lease.load_namespace(name).is_err());
        fs::set_permissions(&record_path, fs::Permissions::from_mode(0o600))?;
        let alias = parent.join("namespace-alias.cbor");
        fs::copy(&record_path, &alias)?;
        assert!(lease.load_namespace("namespace-alias").is_err());
        fs::remove_file(alias)?;
        assert_eq!(lease.load_namespace(name)?, record);
        let renamed = parent.with_file_name("renamed-namespace-pins");
        fs::rename(parent, &renamed)?;
        let parent = renamed.as_path();
        drop(lease);
        // A live pin keeps the lock even after the directory facade is gone.
        assert_eq!(
            conglobate::anchor::RecoveryDirectory::open(parent)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::WouldBlock
        );
        drop(pin); // Persistent mount survives loss of its userspace owner.
        rustix::process::pidfd_send_signal(&holder_identity, rustix::process::Signal::KILL)?;
        let mut poll = [rustix::event::PollFd::new(
            &holder_identity,
            rustix::event::PollFlags::IN,
        )];
        assert_ne!(
            rustix::event::poll(
                &mut poll,
                Some(&rustix::event::Timespec {
                    tv_sec: 5,
                    tv_nsec: 0
                })
            )?,
            0
        );
        assert!(
            poll[0]
                .revents()
                .intersects(rustix::event::PollFlags::IN | rustix::event::PollFlags::HUP)
        );
        drop(holder_identity);
        let lease = conglobate::anchor::RecoveryDirectory::open(parent)?;
        let persisted = lease.load_namespace(record.name().to_str().unwrap())?;
        assert_eq!(persisted, record);
        let pin = conglobate::anchor::NamespacePin::recover(&lease, persisted)?;
        assert_eq!(pin.record().inode(), namespace);
        let restricted = EffectiveCapabilities::without_sys_admin()?;
        let failure = pin.release().unwrap_err();
        let conglobate::anchor::ReleaseFailure::Mounted { pin, .. } = failure else {
            panic!("expected retained namespace mount");
        };
        drop(restricted);
        pin.release()?;
        assert!(conglobate::anchor::NamespacePin::recover(&lease, record.clone()).is_err());
        lease.remove_namespace_record(&record)?;
        assert!(
            lease
                .load_namespace(record.name().to_str().unwrap())
                .is_err()
        );
        println!("PRODUCTION_NAMESPACE_RECORD_PERSISTENCE_PASS");
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        let pin = conglobate::anchor::NamespacePin::create(
            &lease,
            fs::File::open("/proc/self/ns/user")?,
        )?;
        let record = pin.record().clone();
        pin.release()?;
        std::os::unix::fs::symlink("/proc/self/ns/user", parent.join(record.name()))?;
        assert!(conglobate::anchor::NamespacePin::recover(&lease, record.clone()).is_err());
        fs::remove_file(parent.join(record.name()))?;
        drop(lease);
        drop(conglobate::anchor::RecoveryDirectory::open(parent)?);
        assert!(Self::check_lease_in_child(parent)?);
        let output = std::process::Command::new(std::env::current_exe()?)
            .arg("--publish-pin")
            .arg(parent)
            .output()?;
        assert!(output.status.success());
        let name = String::from_utf8(output.stdout)?;
        let lease = conglobate::anchor::RecoveryDirectory::open(parent)?;
        let record = lease.load_namespace(name.trim())?;
        conglobate::anchor::NamespacePin::recover(&lease, record.clone())?.release()?;
        lease.remove_namespace_record(&record)?;
        drop(lease);
        println!("PRODUCTION_NAMESPACE_PUBLISHER_EXIT_RECOVERY_PASS");
        fs::remove_dir(parent)?;
        assert_eq!(fs::read_dir("/proc/self/fd")?.count(), before);
        println!("PRODUCTION_NAMESPACE_PIN_PERSISTENCE_RECOVERY_PASS");
        Ok(())
    }
    fn owned_watched_startup() -> Result<(), Box<dyn std::error::Error>> {
        use std::{
            process::Command,
            time::{Duration, Instant},
        };
        let root = std::path::Path::new("/work/owned-watched-startup");
        fs::create_dir(root)?;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
        let lease = conglobate::anchor::RecoveryDirectory::open(root)?;
        let parent = std::path::Path::new("/work/cgroups");
        for registration in [false, true] {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut command = Command::new(std::env::current_exe()?);
            command.arg("--guardian-peer");
            let (upstream, supervisor) =
                conglobate::process::Process::launch_supervised(command, deadline)?;
            let domain = conglobate::domain::RecordedDomain::create(&lease, parent)?;
            let record = domain.record().clone();
            let mut command = if registration {
                let mut command = Command::new(std::env::current_exe()?);
                command.arg("--stall-registration");
                command
            } else {
                let mut command = Command::new("/usr/bin/sleep");
                command.arg("30");
                command
            };
            command.env_clear();
            let terminator = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(100));
                upstream.terminate_until(deadline).unwrap()
            });
            let started = Instant::now();
            let failure = domain
                .launch_watched(command, &supervisor, deadline)
                .unwrap_err();
            let cancellation = match *failure {
                conglobate::domain::HelperLaunchFailure::Launch {
                    source: conglobate::process::LaunchFailure::Started { source, process },
                    domain,
                } if !registration => {
                    assert_eq!(source.kind(), std::io::ErrorKind::ConnectionAborted);
                    process.terminate_until(deadline)?;
                    domain
                }
                conglobate::domain::HelperLaunchFailure::Registration {
                    source,
                    process,
                    domain,
                } if registration => {
                    assert_eq!(source.kind(), std::io::ErrorKind::ConnectionAborted);
                    process.terminate_until(deadline)?;
                    domain
                }
                other => return Err(format!("unexpected owned startup phase: {other}").into()),
            };
            terminator.join().unwrap();
            assert!(started.elapsed() < Duration::from_secs(2));
            assert_eq!(lease.load_domain(record.name())?, record);
            cancellation
                .retry_until(deadline)
                .map_err(|e| e.to_string())?
                .finish(parent)
                .map_err(|e| e.to_string())?;
        }
        drop(lease);
        fs::remove_dir(root)?;
        println!("PRODUCTION_OWNED_WATCHED_STARTUP_PASS");
        Ok(())
    }
    fn watched_startup() -> Result<(), Box<dyn std::error::Error>> {
        use std::{
            process::Command,
            time::{Duration, Instant},
        };
        let root = std::path::Path::new("/work/watched-startup");
        fs::create_dir(root)?;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
        let lease = conglobate::anchor::RecoveryDirectory::open(root)?;
        let parent = std::path::Path::new("/work/cgroups");
        for registration in [false, true] {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut command = Command::new(std::env::current_exe()?);
            command.arg("--guardian-peer");
            let (upstream, supervisor) =
                conglobate::process::Process::launch_supervised(command, deadline)?;
            let domain = conglobate::domain::CommandDomain::create(&lease, parent)?;
            let record = lease.save_domain(&domain)?;
            let started = Instant::now();
            if registration {
                let mut command = Command::new(std::env::current_exe()?);
                command.arg("--stall-registration");
                let (helper, channel) =
                    conglobate::process::Process::launch_watched(command, &supervisor, deadline)?;
                let terminator = std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(20));
                    upstream.terminate_until(deadline).unwrap()
                });
                let source = channel
                    .register_helper_watched(domain.registration()?, &supervisor, deadline)
                    .unwrap_err();
                assert_eq!(source.kind(), std::io::ErrorKind::ConnectionAborted);
                terminator.join().unwrap();
                helper.terminate_until(deadline)?;
            } else {
                let terminator = std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(20));
                    upstream.terminate_until(deadline).unwrap()
                });
                let mut command = Command::new("/usr/bin/sleep");
                command.arg("30");
                let failure =
                    conglobate::process::Process::launch_watched(command, &supervisor, deadline)
                        .unwrap_err();
                let conglobate::process::LaunchFailure::Started { source, process } = failure
                else {
                    return Err("watched startup did not reach started state".into());
                };
                assert_eq!(source.kind(), std::io::ErrorKind::ConnectionAborted);
                terminator.join().unwrap();
                process.terminate_until(deadline)?;
            }
            assert!(started.elapsed() < Duration::from_secs(2));
            // Neither helper received authorization to execute a recipe.
            domain.cancel_until(deadline)?;
            lease.remove_domain_record(parent, &record)?;
        }
        drop(lease);
        fs::remove_dir(root)?;
        println!("PRODUCTION_WATCHED_STARTUP_PASS");
        Ok(())
    }
    fn wait_for_file(
        path: &std::path::Path,
        deadline: std::time::Instant,
    ) -> Result<(), Box<dyn std::error::Error>> {
        while !path.exists() {
            if std::time::Instant::now() >= deadline {
                return Err("fixture readiness deadline".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        Ok(())
    }
    fn independent_guardian(root: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        use std::{
            process::Command,
            time::{Duration, Instant},
        };
        let reaper = conglobate::process::ChildReaper::install()?;
        let deadline = Instant::now() + Duration::from_secs(10);
        let upstream = conglobate::process::Supervision::from_stdin()?.exchange_until(deadline)?;
        let lease = conglobate::anchor::RecoveryDirectory::open(&root.join("records"))?;
        let parent = std::path::Path::new("/work/cgroups");
        let domain = conglobate::domain::RecordedDomain::create(&lease, parent)?;
        let name = domain.record().name().to_owned();
        let mut command = Command::new(std::env::current_exe()?);
        command.arg("--domain-helper").env_clear();
        let helper = domain
            .launch_watched(command, &upstream, deadline)
            .map_err(|e| e.to_string())?;
        while fs::read_to_string(parent.join(&name).join("cgroup.procs"))?
            .lines()
            .count()
            < 2
        {
            if Instant::now() >= deadline {
                return Err("independent guardian child missing".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        // These files synchronize the test only; they are not the production
        // readiness/admission protocol and are never exposed to recipe code.
        fs::write(root.join("ready.pending"), &name)?;
        fs::rename(root.join("ready.pending"), root.join("ready"))?;
        let (observation, cancellation) =
            conglobate::domain::RegisteredGuardian::new(upstream, helper)
                .supervise_until(deadline, deadline)
                .into_parts();
        assert_eq!(
            observation?,
            conglobate::process::ShutdownTrigger::SupervisorLost
        );
        let (_, children) = cancellation
            .map_err(|e| e.to_string())?
            .reap_until(deadline)
            .map_err(|e| e.to_string())?
            .with_reaper(reaper)
            .drain_until(deadline)
            .map_err(|e| e.to_string())?
            .finish(parent)
            .map_err(|e| e.to_string())?;
        assert!(children.collected() <= 1);
        assert!(!parent.join(name).exists());
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        drop(lease);
        assert_eq!(fs::read_dir(root.join("records"))?.count(), 0);
        fs::write(root.join("done"), b"cleanup complete")?;
        Ok(())
    }
    fn independent_guardian_test() -> Result<(), Box<dyn std::error::Error>> {
        use std::{
            process::Command,
            time::{Duration, Instant},
        };
        let reaper = conglobate::process::ChildReaper::install()?;
        for depart in [false, true] {
            let root = std::path::Path::new(if depart {
                "/work/departed-supervisor"
            } else {
                "/work/closed-supervisor"
            });
            fs::create_dir(root)?;
            fs::create_dir(root.join("records"))?;
            fs::set_permissions(root.join("records"), fs::Permissions::from_mode(0o700))?;
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut command = Command::new(std::env::current_exe()?);
            if depart {
                command.arg("--departing-supervisor").arg(root);
                let child: conglobate::process::Process<conglobate::process::Running> =
                    command.spawn()?.into();
                assert!(child.wait_until(deadline)?.status().success());
            } else {
                command.arg("--independent-guardian").arg(root);
                let (guardian, channel) =
                    conglobate::process::Process::launch_supervised(command, deadline)?;
                Self::wait_for_file(&root.join("ready"), deadline)?;
                assert!(!Self::check_lease_in_child(&root.join("records"))?);
                drop(channel);
                assert!(guardian.wait_until(deadline)?.status().success());
            }
            Self::wait_for_file(&root.join("done"), deadline)?;
            assert!(Self::check_lease_in_child(&root.join("records"))?);
            assert_eq!(fs::read_dir(root.join("records"))?.count(), 0);
            fs::remove_file(root.join("ready"))?;
            fs::remove_file(root.join("done"))?;
            fs::remove_dir(root.join("records"))?;
            fs::remove_dir(root)?;
        }
        // Reverse failure: the supervisor has a stable guardian wait owner,
        // excludes it, then reopens its persisted domain under the recovery lease.
        let root = std::path::Path::new("/work/dead-guardian");
        fs::create_dir(root)?;
        fs::create_dir(root.join("records"))?;
        fs::set_permissions(root.join("records"), fs::Permissions::from_mode(0o700))?;
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut command = Command::new(std::env::current_exe()?);
        command.arg("--independent-guardian").arg(root);
        let (guardian, channel) =
            conglobate::process::Process::launch_supervised(command, deadline)?;
        Self::wait_for_file(&root.join("ready"), deadline)?;
        let name = fs::read_to_string(root.join("ready"))?;
        assert!(!guardian.terminate_until(deadline)?.status().success());
        channel.wait_for_loss_until(deadline)?;
        drop(channel);
        assert!(!root.join("done").exists());
        let lease = conglobate::anchor::RecoveryDirectory::open(&root.join("records"))?;
        let parent = std::path::Path::new("/work/cgroups");
        let cleanup = conglobate::domain::DomainCancellation::recover(&lease, parent, &name)?
            .retry_until(deadline)
            .map_err(|e| e.to_string())?;
        // No direct child wait owner remains. The outer test subreaper adopted
        // the departed supervisor's guardian plus this guardian's helper, and
        // possibly its sleep if the helper could not reap it before cancellation.
        let children = reaper.drain_until(deadline)?;
        assert!((2..=3).contains(&children.collected()));
        cleanup.finish(parent).map_err(|e| e.to_string())?;
        assert!(!parent.join(name).exists());
        drop(lease);
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        fs::remove_file(root.join("ready"))?;
        fs::remove_dir(root.join("records"))?;
        fs::remove_dir(root)?;
        println!("PRODUCTION_INDEPENDENT_GUARDIAN_PASS");
        Ok(())
    }
    fn guardian_drain() -> Result<(), Box<dyn std::error::Error>> {
        use std::{
            process::Command,
            time::{Duration, Instant},
        };
        let reaper = conglobate::process::ChildReaper::install()?;
        let recovery = std::path::Path::new("/work/guardian-drain-recovery");
        let parent = std::path::Path::new("/work/cgroups");
        fs::create_dir(recovery)?;
        fs::set_permissions(recovery, fs::Permissions::from_mode(0o700))?;
        let lease = conglobate::anchor::RecoveryDirectory::open(recovery)?;
        let domain = conglobate::domain::RecordedDomain::create(&lease, parent)?;
        let record = domain.record().clone();
        let mut command = Command::new(std::env::current_exe()?);
        command.arg("--orphan-helper").env_clear();
        let deadline = Instant::now() + Duration::from_secs(5);
        let helper = domain
            .launch(command, deadline)
            .map_err(|e| e.to_string())?;
        helper.supervision().wait_for_loss_until(deadline)?;
        // Closing the helper channel may precede process exit; reap its owned
        // status before invoking the process-wide adopted-child drain.
        let reaped = helper
            .cancel_until(deadline)
            .map_err(|e| e.to_string())?
            .reap_until(deadline)
            .map_err(|e| e.to_string())?;
        assert_eq!(lease.load_domain(record.name())?, record);
        let (source, drain) = reaped
            .with_reaper(reaper)
            .drain_until(Instant::now())
            .unwrap_err()
            .into_parts();
        assert_eq!(source.kind(), std::io::ErrorKind::TimedOut);
        assert_eq!(lease.load_domain(record.name())?, record);
        let drained = drain.drain_until(deadline).map_err(|e| e.to_string())?;
        let path = recovery.join(format!("{}.cbor", record.name()));
        fs::write(&path, [0x86])?;
        let (_, drained) = drained.finish(parent).unwrap_err().into_parts();
        fs::write(&path, record.to_cbor())?;
        let (_, children) = drained.finish(parent).map_err(|e| e.to_string())?;
        assert_eq!(children.collected(), 1);
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        drop(lease);
        fs::remove_dir(recovery)?;
        println!("PRODUCTION_GUARDIAN_DESCENDANT_DRAIN_PASS");
        Ok(())
    }
    fn registered_guardian() -> Result<(), Box<dyn std::error::Error>> {
        use std::{
            process::{Command, Stdio},
            time::{Duration, Instant},
        };
        let parent = std::path::Path::new("/work/cgroups");
        let recovery = std::path::Path::new("/work/guardian-owner-recovery");
        fs::create_dir(recovery)?;
        fs::set_permissions(recovery, fs::Permissions::from_mode(0o700))?;
        let lease = conglobate::anchor::RecoveryDirectory::open(recovery)?;
        for (lose_supervisor, fail_cancel) in [(false, false), (true, false), (true, true)] {
            let domain = conglobate::domain::RecordedDomain::create(&lease, parent)?;
            let record = domain.record().clone();
            let retained = parent.join(record.name()).join("retained-child");
            if fail_cancel {
                fs::create_dir(&retained)?;
            }
            let mut command = Command::new(std::env::current_exe()?);
            command
                .arg("--domain-helper")
                .env_clear()
                .stdout(Stdio::null());
            let deadline = Instant::now() + Duration::from_secs(5);
            let helper = domain
                .launch(command, deadline)
                .map_err(|e| e.to_string())?;
            let mut command = Command::new(std::env::current_exe()?);
            command
                .arg("--guardian-peer")
                .env_clear()
                .stdout(Stdio::null());
            let (peer, channel) =
                conglobate::process::Process::launch_supervised(command, deadline)?;
            let guardian = conglobate::domain::RegisteredGuardian::new(channel, helper);
            // Both cases use the owning production path, not manual watch/kill.
            let peer = if lose_supervisor {
                peer.terminate_until(deadline)?
            } else {
                let (observation, cancellation) = guardian
                    .supervise_until(Instant::now() + Duration::from_millis(20), deadline)
                    .into_parts();
                assert_eq!(
                    observation.unwrap_err().kind(),
                    std::io::ErrorKind::TimedOut
                );
                cancellation
                    .map_err(|e| e.to_string())?
                    .reap_until(deadline)
                    .map_err(|e| e.to_string())?
                    .finish(parent)
                    .map_err(|e| e.to_string())?;
                peer.wait_until(deadline)?;
                assert!(!parent.join(record.name()).exists());
                assert!(lease.load_domain(record.name()).is_err());
                continue;
            };
            assert!(!peer.status().success());
            let (observation, cancellation) =
                guardian.supervise_until(deadline, deadline).into_parts();
            assert_eq!(
                observation?,
                conglobate::process::ShutdownTrigger::SupervisorLost
            );
            let reaping = if fail_cancel {
                let (_, cancellation) = cancellation.unwrap_err().into_parts();
                assert_eq!(lease.load_domain(record.name())?, record);
                fs::remove_dir(retained)?;
                cancellation
                    .retry_until(deadline)
                    .map_err(|e| e.to_string())?
            } else {
                cancellation.map_err(|e| e.to_string())?
            };
            reaping
                .reap_until(deadline)
                .map_err(|e| e.to_string())?
                .finish(parent)
                .map_err(|e| e.to_string())?;
            assert!(!parent.join(record.name()).exists());
            assert!(lease.load_domain(record.name()).is_err());
        }
        drop(lease);
        assert!(Self::check_lease_in_child(recovery)?);
        fs::remove_dir(recovery)?;
        println!("PRODUCTION_REGISTERED_GUARDIAN_PASS");
        Ok(())
    }
    fn contained_helper() -> Result<(), Box<dyn std::error::Error>> {
        use std::{
            process::{Command, Stdio},
            time::{Duration, Instant},
        };
        let parent = std::path::Path::new("/work/cgroups");
        let recovery_path = std::path::Path::new("/work/contained-helper-recovery");
        fs::create_dir(recovery_path)?;
        fs::set_permissions(recovery_path, fs::Permissions::from_mode(0o700))?;
        let lease = conglobate::anchor::RecoveryDirectory::open(recovery_path)?;
        let domain = conglobate::domain::RecordedDomain::create(&lease, parent)?;
        let record = domain.record().clone();
        let path = parent.join(record.name());
        fs::create_dir(path.join("retained-child"))?;
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg("--domain-helper")
            .env_clear()
            .stdout(Stdio::null());
        let deadline = Instant::now() + Duration::from_secs(5);
        let helper = domain
            .launch(command, deadline)
            .map_err(|error| error.to_string())?;
        assert!(!helper.supervision().peer_lost()?);
        while fs::read_to_string(path.join("cgroup.procs"))?
            .lines()
            .count()
            < 2
        {
            if Instant::now() >= deadline {
                return Err("contained helper descendant missing".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let (_, cancellation) = helper.cancel_until(deadline).unwrap_err().into_parts();
        assert!(path.exists());
        assert_eq!(lease.load_domain(record.name())?, record);
        fs::remove_dir(path.join("retained-child"))?;
        let reaping = cancellation
            .retry_until(deadline)
            .map_err(|error| error.to_string())?;
        assert!(!path.exists());
        let reaped = reaping
            .reap_until(deadline)
            .map_err(|error| error.to_string())?;
        let record_path = recovery_path.join(format!("{}.cbor", record.name()));
        fs::write(&record_path, [0x86])?;
        let (_, reaped) = reaped.finish(parent).unwrap_err().into_parts();
        fs::write(&record_path, record.to_cbor())?;
        let _diagnostic_status = reaped.finish(parent).map_err(|error| error.to_string())?;
        assert!(!record_path.exists());
        // Failure after identity exchange must retain the identified wait owner
        // and a cleanup-only domain, even if no membership was acknowledged.
        let domain = conglobate::domain::RecordedDomain::create(&lease, parent)?;
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg("--registration-close")
            .env_clear()
            .stdout(Stdio::null());
        let deadline = Instant::now() + Duration::from_secs(5);
        let failure = domain.launch(command, deadline).unwrap_err();
        let conglobate::domain::HelperLaunchFailure::Registration {
            process, domain, ..
        } = *failure
        else {
            return Err("expected registration failure".into());
        };
        assert!(process.wait_until(deadline)?.status().success());
        domain
            .retry_until(deadline)
            .map_err(|error| error.to_string())?
            .finish(parent)
            .map_err(|error| error.to_string())?;
        // A spawn failure still retains the persisted domain for cleanup.
        let domain = conglobate::domain::RecordedDomain::create(&lease, parent)?;
        let deadline = Instant::now() + Duration::from_secs(5);
        let failure = domain
            .launch(
                Command::new("/nonexistent-conglobate-fixture-helper"),
                deadline,
            )
            .unwrap_err();
        let conglobate::domain::HelperLaunchFailure::Launch {
            source: conglobate::process::LaunchFailure::Before(_),
            domain,
        } = *failure
        else {
            return Err("expected pre-spawn failure".into());
        };
        domain
            .retry_until(deadline)
            .map_err(|error| error.to_string())?
            .finish(parent)
            .map_err(|error| error.to_string())?;
        let domain = conglobate::domain::RecordedDomain::create(&lease, parent)?;
        let mut command = Command::new("/usr/bin/sleep");
        command.arg("30").env_clear();
        let failure = domain
            .launch(command, Instant::now() + Duration::from_millis(20))
            .unwrap_err();
        let conglobate::domain::HelperLaunchFailure::Launch {
            source: conglobate::process::LaunchFailure::Started { source, process },
            domain,
        } = *failure
        else {
            return Err("expected started helper timeout".into());
        };
        assert_eq!(source.kind(), std::io::ErrorKind::TimedOut);
        let teardown = Instant::now() + Duration::from_secs(5);
        process.terminate_until(teardown)?;
        domain
            .retry_until(teardown)
            .map_err(|error| error.to_string())?
            .finish(parent)
            .map_err(|error| error.to_string())?;
        drop(lease);
        assert!(Self::check_lease_in_child(recovery_path)?);
        fs::remove_dir(recovery_path)?;
        println!("PRODUCTION_CONTAINED_HELPER_LIFECYCLE_PASS");
        Ok(())
    }
    fn recorded_domain() -> Result<(), Box<dyn std::error::Error>> {
        use std::{
            process::{Command, Stdio},
            time::{Duration, Instant},
        };
        let parent = std::path::Path::new("/work/cgroups");
        let recovery_path = std::path::Path::new("/work/recorded-domain-recovery");
        fs::create_dir(recovery_path)?;
        fs::set_permissions(recovery_path, fs::Permissions::from_mode(0o700))?;
        let lease = conglobate::anchor::RecoveryDirectory::open(recovery_path)?;
        let domain = conglobate::domain::RecordedDomain::create(&lease, parent)?;
        let record = domain.record().clone();
        let path = parent.join(record.name());
        fs::create_dir(path.join("retained-child"))?;
        let (_, domain) = domain
            .cancel_until(Instant::now() + Duration::from_secs(5))
            .unwrap_err()
            .into_parts();
        assert_eq!(lease.load_domain(record.name())?, record);
        fs::remove_dir(path.join("retained-child"))?;
        domain
            .retry_until(Instant::now() + Duration::from_secs(5))
            .map_err(|failure| failure.to_string())?
            .finish(parent)
            .map_err(|failure| failure.to_string())?;
        let domain = conglobate::domain::RecordedDomain::create(&lease, parent)?;
        let record = domain.record().clone();
        let path = parent.join(record.name());
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg("--domain-helper")
            .env_clear()
            .stdout(Stdio::null());
        let deadline = Instant::now() + Duration::from_secs(5);
        let (child, watch) = conglobate::process::Process::launch_supervised(command, deadline)?;
        let watch = watch.register_helper_until(domain.registration()?, deadline)?;
        while fs::read_to_string(path.join("cgroup.procs"))?
            .lines()
            .count()
            < 2
        {
            if Instant::now() >= deadline {
                return Err("aggregate fixture child missing".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        drop(domain);
        let domain =
            conglobate::domain::DomainCancellation::recover(&lease, parent, record.name())?;
        // Unexpected fixture errors terminate this disposable guest; borrowed
        // recovery owners cannot escape into the probe's static boxed error.
        let cleanup = domain
            .retry_until(deadline)
            .map_err(|failure| failure.to_string())?;
        assert!(!child.wait_until(deadline)?.status().success());
        assert!(watch.peer_lost()?);
        assert!(!path.exists());
        let record_path = recovery_path.join(format!("{}.cbor", record.name()));
        assert!(record_path.exists());
        fs::write(&record_path, [0x86])?;
        let (_, cleanup) = cleanup.finish(parent).unwrap_err().into_parts();
        fs::write(&record_path, record.to_cbor())?;
        cleanup
            .finish(parent)
            .map_err(|failure| failure.to_string())?;
        assert!(!record_path.exists());
        drop(lease);
        assert!(Self::check_lease_in_child(recovery_path)?);
        fs::remove_dir(recovery_path)?;
        println!("PRODUCTION_RECORDED_DOMAIN_LIFECYCLE_PASS");
        Ok(())
    }
    fn command_domain() -> Result<(), Box<dyn std::error::Error>> {
        use std::{
            process::{Command, Stdio},
            time::{Duration, Instant},
        };
        let parent = std::path::Path::new("/work/cgroups");
        let recovery_path = std::path::Path::new("/work/domain-recovery");
        fs::create_dir(recovery_path)?;
        fs::set_permissions(recovery_path, fs::Permissions::from_mode(0o700))?;
        let lease = conglobate::anchor::RecoveryDirectory::open(recovery_path)?;

        let empty = conglobate::domain::CommandDomain::create(&lease, parent)?;
        drop(lease);
        assert_eq!(
            conglobate::anchor::RecoveryDirectory::open(recovery_path)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::WouldBlock
        );
        assert!(!Self::check_lease_in_child(recovery_path)?);
        empty.cancel_until(Instant::now() + Duration::from_secs(5))?;
        let lease = conglobate::anchor::RecoveryDirectory::open(recovery_path)?;
        let domain = conglobate::domain::CommandDomain::create(&lease, parent)?;
        assert!(domain.registration().is_err());
        let record = lease.save_domain(&domain)?;
        assert_eq!(
            lease.save_domain(&domain).unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert_eq!(lease.load_domain(record.name())?, record);
        assert!(lease.remove_domain_record(parent, &record).is_err());
        let permit = domain.registration()?;
        let record_path = recovery_path.join(format!("{}.cbor", record.name()));
        fs::write(&record_path, [0x86])?;
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg("--domain-helper")
            .env_clear()
            .stdout(Stdio::null());
        let deadline = Instant::now() + Duration::from_secs(5);
        let (child, watch) = conglobate::process::Process::launch_supervised(command, deadline)?;
        assert!(watch.register_helper_until(permit, deadline).is_err());
        assert!(!child.wait_until(deadline)?.status().success());
        fs::write(&record_path, record.to_cbor())?;
        assert!(domain.registration().is_ok());
        println!("PRODUCTION_DOMAIN_REGISTRATION_RECORD_GATE_PASS");

        let other_path = std::path::Path::new("/work/other-recovery");
        fs::create_dir(other_path)?;
        fs::set_permissions(other_path, fs::Permissions::from_mode(0o700))?;
        let other_lease = conglobate::anchor::RecoveryDirectory::open(other_path)?;
        assert!(other_lease.save_domain(&domain).is_err());
        assert_eq!(fs::read_dir(other_path)?.count(), 0);
        drop(other_lease);
        fs::remove_dir(other_path)?;

        let wrong_parent = parent.join("wrong-parent");
        fs::create_dir(&wrong_parent)?;
        assert!(lease.recover_domain(&wrong_parent, record.name()).is_err());
        assert!(lease.remove_domain_record(&wrong_parent, &record).is_err());
        fs::remove_dir(wrong_parent)?;

        let path = fs::read_dir(parent)?
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("conglobate-command-")
            })
            .ok_or("missing fixture domain")?
            .path();
        fs::create_dir(path.join("retained-child"))?;
        let failure = domain
            .cancel_until(Instant::now() + Duration::from_secs(5))
            .unwrap_err();
        let (_, domain) = failure.into_parts();
        drop(lease);
        assert_eq!(
            conglobate::anchor::RecoveryDirectory::open(recovery_path)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::WouldBlock
        );
        assert!(path.exists());
        fs::remove_dir(path.join("retained-child"))?;
        let private = conglobate::isolation::PrivateMountNamespace::enter()?;
        let (_, domain) = domain
            .cancel_until(Instant::now() + Duration::from_secs(5))
            .unwrap_err()
            .into_parts();
        private.restore()?;
        assert!(path.exists());
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg("--domain-helper")
            .env_clear()
            .stdout(Stdio::null());
        let deadline = Instant::now() + Duration::from_secs(5);
        let (child, watch) = conglobate::process::Process::launch_supervised(command, deadline)?;
        let watch = watch.register_helper_until(domain.registration()?, deadline)?;
        assert!(!watch.peer_lost()?);
        while fs::read_to_string(path.join("cgroup.procs"))?
            .lines()
            .count()
            < 2
        {
            if Instant::now() >= deadline {
                return Err("fixture descendant not created".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let mut supervisor_command = Command::new(std::env::current_exe()?);
        supervisor_command
            .arg("--guardian-peer")
            .env_clear()
            .stdout(Stdio::null());
        let (supervisor_peer, supervisor_watch) =
            conglobate::process::Process::launch_supervised(supervisor_command, deadline)?;
        let guardian = conglobate::process::GuardianWatch::new(&supervisor_watch, &watch);
        assert_eq!(
            guardian
                .wait_until(Instant::now() + Duration::from_millis(20))
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::TimedOut
        );
        let terminator = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            supervisor_peer.terminate_until(deadline)
        });
        assert_eq!(
            guardian.wait_until(deadline)?,
            conglobate::process::ShutdownTrigger::SupervisorLost
        );
        terminator.join().expect("fixture terminator thread")?;
        println!("PRODUCTION_GUARDIAN_SUPERVISOR_LOSS_PASS");
        drop(domain);
        let recovered_lease = conglobate::anchor::RecoveryDirectory::open(recovery_path)?;
        assert_eq!(recovered_lease.load_domain(record.name())?, record);
        let domain = recovered_lease.recover_domain(parent, record.name())?;
        drop(recovered_lease);
        domain.cancel_until(deadline)?;
        assert!(!child.wait_until(deadline)?.status().success());
        assert!(!path.exists());
        assert!(watch.peer_lost()?);
        assert_eq!(
            guardian.wait_until(deadline)?,
            conglobate::process::ShutdownTrigger::BothLost
        );
        let lease = conglobate::anchor::RecoveryDirectory::open(recovery_path)?;
        assert!(lease.recover_domain(parent, record.name()).is_err());
        lease.remove_domain_record(parent, &record)?;
        assert!(lease.load_domain(record.name()).is_err());
        let replaced = conglobate::domain::CommandDomain::create(&lease, parent)?;
        let replaced_record = lease.save_domain(&replaced)?;
        let replaced_path = parent.join(replaced_record.name());
        drop(replaced);
        fs::remove_dir(&replaced_path)?;
        fs::create_dir(&replaced_path)?;
        fs::set_permissions(&replaced_path, fs::Permissions::from_mode(0o700))?;
        assert!(
            lease
                .recover_domain(parent, replaced_record.name())
                .is_err()
        );
        assert!(
            lease
                .remove_domain_record(parent, &replaced_record)
                .is_err()
        );
        fs::remove_dir(&replaced_path)?;
        lease.remove_domain_record(parent, &replaced_record)?;
        println!("PRODUCTION_COMMAND_DOMAIN_REPLACEMENT_REJECTED_PASS");
        println!("PRODUCTION_COMMAND_DOMAIN_RECORD_RECOVERY_PASS");
        let abandoned = conglobate::domain::CommandDomain::create(&lease, parent)?;
        assert!(abandoned.registration().is_err());
        let abandoned_record = lease.save_domain(&abandoned)?;
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg("--registration-close")
            .env_clear()
            .stdout(Stdio::null());
        let deadline = Instant::now() + Duration::from_secs(5);
        let (child, watch) = conglobate::process::Process::launch_supervised(command, deadline)?;
        assert!(
            watch
                .register_helper_until(abandoned.registration()?, deadline)
                .is_err()
        );
        assert!(child.wait_until(deadline)?.status().success());
        abandoned.cancel_until(deadline)?;
        lease.remove_domain_record(parent, &abandoned_record)?;
        println!("PRODUCTION_COMMAND_DOMAIN_MISSING_ACK_PASS");
        println!("PRODUCTION_COMMAND_DOMAIN_REGISTER_PASS");
        drop(lease);
        drop(conglobate::anchor::RecoveryDirectory::open(recovery_path)?);
        assert!(Self::check_lease_in_child(recovery_path)?);
        let published = Command::new(std::env::current_exe()?)
            .arg("--publish-domain")
            .arg(recovery_path)
            .arg(parent)
            .output()?;
        assert!(published.status.success());
        let published_name = String::from_utf8(published.stdout)?;
        let lease = conglobate::anchor::RecoveryDirectory::open(recovery_path)?;
        let published_record = lease.load_domain(published_name.trim())?;
        lease
            .recover_domain(parent, published_record.name())?
            .cancel_until(Instant::now() + Duration::from_secs(5))?;
        lease.remove_domain_record(parent, &published_record)?;
        drop(lease);
        fs::remove_dir(recovery_path)?;
        println!("PRODUCTION_COMMAND_DOMAIN_PUBLISHER_EXIT_PASS");
        println!("PRODUCTION_COMMAND_DOMAIN_LEASE_PASS");
        println!("PRODUCTION_COMMAND_DOMAIN_CANCEL_PASS");
        Ok(())
    }
    fn check_lease_in_child(path: &std::path::Path) -> std::io::Result<bool> {
        Ok(std::process::Command::new("/idmap-probe")
            .arg("--check-lease")
            .arg(path)
            .env_clear()
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?
            .success())
    }
}
#[derive(Debug)]
struct EffectiveCapabilities(rustix::thread::CapabilitySets);
impl EffectiveCapabilities {
    fn without_sys_admin() -> std::io::Result<Self> {
        let saved = rustix::thread::capabilities(None)?;
        let mut restricted = saved;
        restricted
            .effective
            .remove(rustix::thread::CapabilitySet::SYS_ADMIN);
        rustix::thread::set_capabilities(None, restricted)?;
        Ok(Self(saved))
    }

    fn clear() -> std::io::Result<Self> {
        let saved = rustix::thread::capabilities(None)?;
        let mut restricted = saved;
        restricted.effective = rustix::thread::CapabilitySet::empty();
        rustix::thread::set_capabilities(None, restricted)?;
        Ok(Self(saved))
    }
}
impl Drop for EffectiveCapabilities {
    fn drop(&mut self) {
        rustix::thread::set_capabilities(None, self.0).expect("restore probe capabilities");
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Probe::run()
}
