#[cfg(test)]
mod recovery_controls {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn interrupted_recovery_can_restart() {
        use std::process::{Command, Stdio};
        use std::os::unix::process::ExitStatusExt;
        const TEST: &str = "assemble::recovery_controls::interrupted_recovery_can_restart";
        let assembler = Assembler { control: Control::open().unwrap() };
        if let Ok(gate) = std::env::var("CARAPACE_RECOVERY_GATE") {
            let name = std::env::var("CARAPACE_RECOVERY_NAME").unwrap();
            let _ = assembler.activate_inner(&name, 8, Zero, &mut |stage| {
                if stage == ActivationStage::RecoveryRemoved {
                    std::fs::write(&gate, b"removed")?;
                    std::thread::sleep(Duration::from_secs(10));
                    panic!("parent did not terminate recovery fixture");
                }
                Ok(())
            });
            panic!("recovery gate not reached");
        }
        let name = format!("CARAPACE-recovery-death-{}", std::process::id());
        assembler.control.create(&name).unwrap();
        let gate = format!("/tmp/{name}.ready");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env("CARAPACE_RECOVERY_GATE", &gate)
            .env("CARAPACE_RECOVERY_NAME", &name)
            .stdin(Stdio::null()).spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !std::path::Path::new(&gate).exists() {
            assert!(child.try_wait().unwrap().is_none());
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(assembler.control.by_name(&name).is_err());
        child.kill().unwrap();
        assert_eq!(child.wait().unwrap().signal(), Some(9));
        std::fs::remove_file(gate).unwrap();
        let lock = File::open("/dev/mapper/control").unwrap();
        lock.try_lock().expect("dead recoverer retained publication lock");
        drop(lock);
        let file = assembler.activate(&name, 8, Zero).unwrap();
        let mut bytes = [1; 512];
        file.read_exact_at(&mut bytes, 0).unwrap();
        assert_eq!(bytes, [0; 512]);
        drop(file);
        let deadline = Instant::now() + Duration::from_secs(3);
        while assembler.control.by_name(&name).is_ok() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        println!("RECOVERY_DEATH_AFTER_REMOVE_RESTART_PASS");
    }

    #[test]
    fn preserves_held_inactive_and_mismatched_active_targets() {
        let assembler = Assembler { control: Control::open().unwrap() };
        let name = format!("CARAPACE-held-incomplete-{}", std::process::id());
        let id = assembler.control.create(&name).unwrap();
        let table = Table::<4096>::default()
            .add(Span::new(0, 8), &Zero, assembler.control.version(Zero::NAME).unwrap()).unwrap();
        assembler.control.load(id, table, true).unwrap();
        let holder = File::open(format!("/dev/dm-{}", id.minor())).unwrap();
        assert!(assembler.control.status(id).unwrap().open_count > 0);
        assert!(assembler.activate(&name, 8, Zero).is_err());
        assert_eq!(assembler.control.by_name(&name).unwrap().0, id);
        assert_eq!(DevId::try_from(holder.metadata().unwrap()).unwrap(), id);
        assert!(assembler.control.status(id).unwrap().flags & Status::INACTIVE_TABLE != 0);
        println!("RECOVERY_HELD_INACTIVE_PRESERVED");
        drop(holder);
        let adopted = assembler.activate(&name, 8, Zero).unwrap();
        // A matching active mapping must not be deleted just because another
        // attempted assembly asks for a different table span.
        let identity = DevId::try_from(adopted.metadata().unwrap()).unwrap();
        assert!(assembler.activate(&name, 16, Zero).is_err());
        assert_eq!(assembler.control.by_name(&name).unwrap().0, identity);
        let mut bytes = [1; 512];
        adopted.read_exact_at(&mut bytes, 0).unwrap();
        assert_eq!(bytes, [0; 512]);
        println!("RECOVERY_MISMATCHED_ACTIVE_PRESERVED");
        drop(adopted);
        let deadline = Instant::now() + Duration::from_secs(3);
        while assembler.control.by_name(&name).is_ok() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
