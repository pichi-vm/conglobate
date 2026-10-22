// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]
use conglobate::policy::{FilesystemTarget, LabelPolicy, LabelScope, PinDirectory};
use std::{fs, os::unix::fs::MetadataExt};

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
            .is_some_and(|argument| argument == "--check-lease")
        {
            let path = arguments.get(2).ok_or("missing lease path")?;
            drop(conglobate::anchor::RecoveryDirectory::open(
                std::path::Path::new(path),
            )?);
            return Ok(());
        }
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
        let restricted = EffectiveCapabilities::clear()?;
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
        let record_path = parent.join(format!("{name}.cbor"));
        fs::write(&record_path, [0x84])?;
        assert!(lease.load_namespace(name).is_err());
        fs::write(&record_path, record.to_cbor())?;
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
        let restricted = EffectiveCapabilities::clear()?;
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
        fs::remove_dir(parent)?;
        assert_eq!(fs::read_dir("/proc/self/fd")?.count(), before);
        println!("PRODUCTION_NAMESPACE_PIN_PERSISTENCE_RECOVERY_PASS");
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
