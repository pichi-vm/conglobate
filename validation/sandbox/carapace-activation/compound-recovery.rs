// Test-only dependency-lifetime matrix in a copied dependency, inside a guest.
#[cfg(test)]
mod compound_recovery {
    use super::*;
    use crate::image::{Plan as _, Planned};
    use std::{process::{Command, Stdio}, time::{Duration, Instant}};
    use std::os::unix::process::ExitStatusExt;

    fn target(origin: &File, cow: &File) -> Snapshot {
        Snapshot::from(SnapshotParameters {
            origin: DevId::try_from(origin.metadata().unwrap()).unwrap(),
            cow: DevId::try_from(cow.metadata().unwrap()).unwrap(),
            persistence: Persistence::Persistent(Persistent::Standard),
            chunk_size: BlockSize::default(), discard: Discard::Disabled,
        })
    }
    fn store(directory: &std::path::Path, name: &str, block: usize, byte: u8) -> File {
        let mut bytes = vec![0; 8192];
        bytes[block*4096..(block+1)*4096].fill(byte);
        let raw = directory.join(format!("{name}.raw"));
        std::fs::write(&raw, bytes).unwrap();
        let mut planned = Planned::plan(File::open(raw).unwrap()).unwrap();
        let path = directory.join(format!("{name}.cow"));
        let mut file = File::create(&path).unwrap();
        std::io::copy(&mut planned, &mut file).unwrap();
        drop(file);
        File::open(path).unwrap()
    }
    fn absent(assembler: &Assembler, name: &str) {
        let deadline = Instant::now()+Duration::from_secs(3);
        while assembler.control.by_name(name).is_ok() {
            assert!(Instant::now()<deadline, "mapping did not release: {name}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn contents(file: &File, first: u8, second: u8) {
        let mut bytes = [0;8192];
        file.read_exact_at(&mut bytes,0).unwrap();
        assert_eq!(&bytes[..4096], &[first;4096]);
        assert_eq!(&bytes[4096..], &[second;4096]);
    }
    #[test]
    fn snapshot_dependencies_survive_recovery() {
        const TEST: &str = "assemble::compound_recovery::snapshot_dependencies_survive_recovery";
        let assembler = Assembler {control: Control::open().unwrap()};
        if let Ok(slot) = std::env::var("CARAPACE_COMPOUND_REUSER") {
            let directory = std::path::PathBuf::from(std::env::var("CARAPACE_COMPOUND_DIRECTORY").unwrap());
            std::fs::write(directory.join(format!("reuser-{slot}-ready")),b"ready").unwrap();
            let deadline = Instant::now()+Duration::from_secs(5);
            while !directory.join("reusers-start").exists() {
                assert!(Instant::now()<deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
            let origin = File::open(std::env::var("CARAPACE_COMPOUND_ORIGIN").unwrap()).unwrap();
            let cow = File::open(std::env::var("CARAPACE_COMPOUND_COW").unwrap()).unwrap();
            let name = std::env::var("CARAPACE_COMPOUND_NAME").unwrap();
            let upper = assembler.activate(&name,16,target(&origin,&cow)).unwrap();
            contents(&upper,0x41,0x62);
            let id = DevId::try_from(upper.metadata().unwrap()).unwrap();
            std::fs::write(directory.join(format!("reuser-{slot}-held")),id.to_string()).unwrap();
            let deadline = Instant::now()+Duration::from_secs(5);
            while !directory.join(format!("reuser-{slot}-release")).exists() {
                assert!(Instant::now()<deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
            contents(&upper,0x41,0x62);
            return;
        }
        if let Ok(stage) = std::env::var("CARAPACE_COMPOUND_STAGE") {
            let origin = File::open(std::env::var("CARAPACE_COMPOUND_ORIGIN").unwrap()).unwrap();
            let cow = File::open(std::env::var("CARAPACE_COMPOUND_COW").unwrap()).unwrap();
            let name = std::env::var("CARAPACE_COMPOUND_NAME").unwrap();
            let gate = std::env::var("CARAPACE_COMPOUND_GATE").unwrap();
            assembler.activate_inner(&name,16,target(&origin,&cow),&mut |observed| {
                if format!("{observed:?}")==stage {
                    std::fs::write(&gate,b"ready")?;
                    std::thread::sleep(Duration::from_secs(10));
                    panic!("parent failed to terminate owned child");
                }
                Ok(())
            }).unwrap();
            panic!("requested activation stage not reached");
        }
        let directory = tempfile::tempdir().unwrap();
        let prefix = format!("CARAPACE-compound-{}",std::process::id());
        let zero_name = format!("{prefix}-zero");
        let lower_name = format!("{prefix}-lower");
        let origin = assembler.activate(&zero_name,16,Zero).unwrap();
        let base_store = store(directory.path(),"base",0,0x41);
        let base_bytes = std::fs::read(directory.path().join("base.cow")).unwrap();
        let base_device = Assembler::resolve(&base_store).unwrap();
        let lower = assembler.activate(&lower_name,16,target(&origin,&base_device)).unwrap();
        let lower_id = DevId::try_from(lower.metadata().unwrap()).unwrap();
        drop(origin); drop(base_device);
        contents(&lower,0x41,0);
        for stage in ["Created","Loaded","Resumed","Opened"] {
            let name = format!("{prefix}-{stage}");
            let cow_store = store(directory.path(),stage,1,0x62);
            let before = std::fs::read(directory.path().join(format!("{stage}.cow"))).unwrap();
            let cow = Assembler::resolve(&cow_store).unwrap();
            let cow_id = DevId::try_from(cow.metadata().unwrap()).unwrap();
            let snapshot = target(&lower,&cow);
            let gate = directory.path().join(format!("{stage}.ready"));
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args(["--exact",TEST,"--nocapture"])
                .env("CARAPACE_COMPOUND_STAGE",stage).env("CARAPACE_DEATH_STAGE",stage)
                .env("CARAPACE_COMPOUND_NAME",&name).env("CARAPACE_COMPOUND_GATE",&gate)
                .env("CARAPACE_COMPOUND_ORIGIN",format!("/dev/dm-{}",lower_id.minor()))
                .env("CARAPACE_COMPOUND_COW",format!("/dev/loop{}",cow_id.minor()))
                .stdin(Stdio::null()).spawn().unwrap();
            let deadline = Instant::now()+Duration::from_secs(5);
            while !gate.exists() {
                assert!(child.try_wait().unwrap().is_none());
                assert!(Instant::now()<deadline);
                std::thread::sleep(Duration::from_millis(10));
            }
            child.kill().unwrap(); assert_eq!(child.wait().unwrap().signal(),Some(9));
            contents(&lower,0x41,0);
            if stage == "Loaded" {
                let mut reusers = Vec::new();
                for slot in 0..2 {
                    reusers.push(Command::new(std::env::current_exe().unwrap())
                        .args(["--exact",TEST,"--nocapture"])
                        .env("CARAPACE_COMPOUND_REUSER",slot.to_string())
                        .env("CARAPACE_COMPOUND_DIRECTORY",directory.path())
                        .env("CARAPACE_COMPOUND_NAME",&name)
                        .env("CARAPACE_COMPOUND_ORIGIN",format!("/dev/dm-{}",lower_id.minor()))
                        .env("CARAPACE_COMPOUND_COW",format!("/dev/loop{}",cow_id.minor()))
                        .stdin(Stdio::null()).spawn().unwrap());
                }
                for phase in ["ready","held"] {
                    let deadline = Instant::now()+Duration::from_secs(5);
                    loop {
                        assert!(reusers.iter_mut().all(|p| p.try_wait().unwrap().is_none()));
                        if (0..2).all(|slot| directory.path().join(format!("reuser-{slot}-{phase}")).exists()) { break; }
                        assert!(Instant::now()<deadline);
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    if phase == "ready" { std::fs::write(directory.path().join("reusers-start"),b"go").unwrap(); }
                }
                let first = std::fs::read_to_string(directory.path().join("reuser-0-held")).unwrap();
                let second = std::fs::read_to_string(directory.path().join("reuser-1-held")).unwrap();
                assert_eq!(first,second);
                let shared_id = assembler.control.by_name(&name).unwrap().0;
                assert_eq!(shared_id.to_string(),first);
                std::fs::write(directory.path().join("reuser-0-release"),b"release").unwrap();
                assert!(reusers[0].wait().unwrap().success());
                assert!(reusers[1].try_wait().unwrap().is_none());
                assert_eq!(assembler.control.by_name(&name).unwrap().0,shared_id);
                let independent = File::open(format!("/dev/dm-{}",shared_id.minor())).unwrap();
                contents(&independent,0x41,0x62); drop(independent);
                contents(&lower,0x41,0);
                std::fs::write(directory.path().join("reuser-1-release"),b"release").unwrap();
                assert!(reusers[1].wait().unwrap().success());
                absent(&assembler,&name);
                contents(&lower,0x41,0);
                println!("COMPOUND_CONCURRENT_RECOVERY_SAME_MAPPING_PASS");
                println!("COMPOUND_FIRST_READER_RELEASE_PRESERVES_SECOND_PASS");
            }
            let upper = assembler.activate(&name,16,snapshot).unwrap();
            drop(cow); // Only the active table now retains this loop attachment.
            contents(&upper,0x41,0x62);
            assert_eq!(assembler.control.by_name(&lower_name).unwrap().0,lower_id);
            drop(upper); absent(&assembler,&name);
            contents(&lower,0x41,0);
            assert_eq!(std::fs::read(directory.path().join(format!("{stage}.cow"))).unwrap(),before);
            assert_eq!(std::fs::read(directory.path().join("base.cow")).unwrap(),base_bytes);
            println!("COMPOUND_RECOVERY_SHARED_READER_AND_BYTES_PASS {stage}");
        }
        drop(lower); absent(&assembler,&lower_name); absent(&assembler,&zero_name);
        println!("COMPOUND_RECOVERY_DEPENDENCIES_RELEASED_PASS");
    }
}
