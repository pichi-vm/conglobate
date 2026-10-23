// SPDX-License-Identifier: Apache-2.0
use conglobate::process::CommandSyscalls;
use std::process::Command;

/// Disposable VM fixture retaining namespace authority in both test arms.
#[derive(Debug)]
pub(super) struct CloneFilter;
impl CloneFilter {
    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        assert!(
            Command::new("/usr/bin/clone-probe")
                .arg("control")
                .status()?
                .success()
        );
        CommandSyscalls::compile()?.apply()?;
        assert!(
            Command::new("/usr/bin/clone-probe")
                .arg("filtered")
                .status()?
                .success()
        );
        println!("NATIVE_CLONE_NAMESPACE_MASK_PASS");
        Ok(())
    }
}
