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
    fn run() -> Result<(), Box<dyn std::error::Error>> {
        if !std::path::Path::new("/conglobate-disposable-validation-vm").exists() {
            return Err("disposable VM marker required".into());
        }
        let arguments = std::env::args_os().collect::<Vec<_>>();
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
        let domain = conglobate::domain::RecordedDomain::recover(&lease, parent, record.name())?;
        // Unexpected fixture errors terminate this disposable guest; borrowed
        // recovery owners cannot escape into the probe's static boxed error.
        let cleanup = domain
            .cancel_until(deadline)
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
