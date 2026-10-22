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
        Ok(())
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
