// Test-only addition to a copied pinned dependency. No production changes.
#[cfg(test)]
mod crash_validation {
    use super::*;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    #[test]
    fn kernel_abrupt_death_matrix() {
        const TEST: &str = "assemble::crash_validation::kernel_abrupt_death_matrix";
        let assembler = Assembler { control: Control::open().unwrap() };
        if let Ok(stage) = std::env::var("CARAPACE_DEATH_STAGE") {
            let name = std::env::var("CARAPACE_DEATH_NAME").unwrap();
            let gate = std::env::var("CARAPACE_DEATH_GATE").unwrap();
            let _ = assembler.activate_inner(&name, 8, Zero, &mut |observed| {
                if format!("{observed:?}") == stage {
                    std::fs::write(&gate, b"ready")?;
                    // Bounded gate; the parent kills this process before rollback.
                    std::thread::sleep(Duration::from_secs(10));
                    panic!("parent did not terminate the fixture child");
                }
                Ok(())
            });
            panic!("activation did not reach requested boundary");
        }
        for stage in ["Created", "Resumed"] {
            let name = format!("CARAPACE-death-{}-{stage}", std::process::id());
            let gate = format!("/tmp/{name}.ready");
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env("CARAPACE_DEATH_STAGE", stage)
                .env("CARAPACE_DEATH_NAME", &name)
                .env("CARAPACE_DEATH_GATE", &gate)
                .stdin(Stdio::null()).spawn().unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while !std::path::Path::new(&gate).exists() {
                assert!(child.try_wait().unwrap().is_none());
                assert!(Instant::now() < deadline, "activation gate timeout");
                std::thread::sleep(Duration::from_millis(10));
            }
            child.kill().unwrap();
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(child.wait().unwrap().signal(), Some(9));
            std::fs::remove_file(gate).unwrap();
            let (id, _) = assembler.control.by_name(&name).unwrap();
            let attempt = assembler.activate(&name, 8, Zero);
            if stage == "Created" {
                let error = attempt.unwrap_err();
                assert!(assembler.control.by_name(&name).is_ok());
                println!("CREATED_DEATH_STALE_MAPPING_BLOCKS_REUSE {error}");
                // Only this private test name is removed. This is fixture cleanup,
                // not an ownership-safe production recovery algorithm.
                assembler.control.remove(id, false).unwrap();
            } else {
                let file = attempt.unwrap();
                assert_eq!(DevId::try_from(file.metadata().unwrap()).unwrap(), id);
                drop(file);
                let deadline = Instant::now() + Duration::from_secs(3);
                while assembler.control.by_name(&name).is_ok() {
                    assert!(Instant::now() < deadline, "adopted mapping leaked");
                    std::thread::sleep(Duration::from_millis(10));
                }
                println!("RESUMED_DEATH_REUSE_AND_DEFERRED_CLEANUP_PASS");
            }
        }
    }
}
