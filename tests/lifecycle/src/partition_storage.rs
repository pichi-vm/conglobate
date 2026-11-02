// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::{
    contract::plan::Format,
    disk::{Architecture, Disk},
    execution::HelperProgram,
    formatter::{CapturedFormatter, RootFormat},
    step::SetupBudget,
    storage::{Created, Mapping, PartitionedSnapshot, ReadOnlyChain, Workspace},
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    num::NonZeroU64,
    path::Path,
    process::Command,
    sync::Arc,
};

#[derive(Debug)]
pub(super) struct PartitionStorage;
impl PartitionStorage {
    fn node(id: devmap_linux::DevId) -> String {
        format!("/dev/dm-{}", id.minor())
    }

    fn command(program: &str, arguments: &[&str]) -> io::Result<()> {
        let status = Command::new(program).args(arguments).status()?;
        if !status.success() {
            return Err(io::Error::other(format!("{program}: {status}")));
        }
        Ok(())
    }

    fn contents(
        filesystem: &str,
        root_mib: u64,
        partitions: &PartitionedSnapshot<'_, '_>,
        previous: Option<&str>,
        next: Option<&str>,
    ) -> io::Result<()> {
        for (kind, device) in [
            (filesystem, partitions.root_id()),
            ("vfat", partitions.esp_id().expect("formatted ESP")),
        ] {
            let node = Self::node(device);
            let mut file = OpenOptions::new().read(true).write(true).open(&node)?;
            assert_eq!(rustix::fs::ioctl_blksszget(&file)?, 4096);
            let size = file.seek(SeekFrom::End(0))?;
            assert_eq!(
                size,
                if kind == "vfat" { 257 } else { root_mib } * 1024 * 1024
            );
            assert_eq!(file.read(&mut [0; 1])?, 0);
            assert!(
                file.write_all(&[0]).is_err(),
                "partition write escaped its bounds"
            );
            drop(file);
            let target = format!("/work/{kind}");
            fs::create_dir_all(&target)?;
            Self::command(
                "mount",
                &[
                    "-t",
                    kind,
                    "-o",
                    if kind == "xfs" { "nouuid" } else { "defaults" },
                    &node,
                    &target,
                ],
            )?;
            let path = Path::new(&target).join("marker");
            if let Some(previous) = previous {
                assert_eq!(fs::read_to_string(&path)?, previous);
            } else {
                assert!(!path.exists());
            }
            if let Some(next) = next {
                fs::write(path, next)?;
            }
            Self::command("umount", &[&target])?;
            if kind == "ext4" {
                Self::command("e2fsck", &["-fn", &node])?;
            } else if kind == "xfs" {
                Self::command("xfs_repair", &["-n", &node])?;
            } else {
                Self::command("fsck.fat", &["-n", &node])?;
            }
        }
        partitions.validate()
    }

    fn digest(id: devmap_linux::DevId, bytes: u64) -> io::Result<[u8; 32]> {
        let mut file = File::open(Self::node(id))?.take(bytes);
        let mut digest = Sha256::new();
        let mut buffer = [0; 65536];
        let mut copied = 0;
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            digest.update(&buffer[..count]);
            copied += count as u64;
        }
        assert_eq!(copied, bytes);
        Ok(digest.finalize().into())
    }

    pub(super) fn run(busy: bool) -> Result<(), Box<dyn std::error::Error>> {
        for filesystem in ["ext4", "xfs"] {
            Self::filesystem(filesystem, busy)?;
            if busy {
                break;
            }
        }
        Ok(())
    }

    fn filesystem(filesystem: &str, busy: bool) -> Result<(), Box<dyn std::error::Error>> {
        let root_mib = if filesystem == "ext4" { 32 } else { 512 };
        let baseline = super::chain_storage::ChainStorage::mappings()?;
        let workspace = Workspace::create()?;
        let sectors = NonZeroU64::new(carapace::ZERO_SIZE_SECTORS).unwrap();
        let origin = Mapping::<Created>::create()?.zero(sectors)?;
        let mut scratch = workspace.scratch()?;
        scratch.set_len(128 * 1024 * 1024)?;
        let snapshot =
            Mapping::<Created>::create()?.snapshot(&origin, scratch.attach()?, sectors)?;
        let spec: Format = serde_saphyr::from_str(&format!(
            "fs: {filesystem}\nsize: {root_mib}MiB\nlabel: root-test\nesp: 257MiB\n"
        ))?;
        let format = RootFormat::from(&spec);
        let bytes = format.layout()?.bytes();
        let (runtime, snapshot) = CapturedFormatter::for_filesystem(&spec)?
            .stage(&workspace)?
            .format(
                snapshot,
                format,
                &HelperProgram::current()?,
                SetupBudget::begin(),
            )?
            .wait()?
            .into_parts()?;
        runtime.close()?;
        let snapshot_id = snapshot.id();
        let mut disk_file = File::open(Self::node(snapshot_id))?;
        let disk = Disk::read(&mut disk_file, carapace::ZERO_SIZE_BYTES)?;
        let root = disk.root(Architecture::current()?)?;
        let esp = disk.esp()?.unwrap();
        assert_eq!(root.sectors * 512, root_mib * 1024 * 1024);
        assert_eq!(esp.sectors * 512, 257 * 1024 * 1024);
        assert!(root.start >= esp.start + esp.sectors);
        // Check filesystem signatures at full-disk offsets, independently of
        // the view's zero-based filesystem reads and the GPT parser.
        if filesystem == "ext4" {
            disk_file.seek(SeekFrom::Start(root.start * 512 + 1024 + 56))?;
            let mut magic = [0; 2];
            disk_file.read_exact(&mut magic)?;
            assert_eq!(magic, [0x53, 0xef]);
        }
        disk_file.seek(SeekFrom::Start(esp.start * 512 + 82))?;
        let mut fat = [0; 8];
        disk_file.read_exact(&mut fat)?;
        assert_eq!(&fat, b"FAT32   ");
        drop(disk_file);
        let partitions = PartitionedSnapshot::open(snapshot)?;
        if busy {
            let mut holder = File::open(Self::node(partitions.esp_id().unwrap()))?;
            let error = partitions.into_snapshot().unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::ResourceBusy);
            assert!(Path::new(&Self::node(snapshot_id)).exists());
            holder.seek(SeekFrom::Start(82))?;
            holder.read_exact(&mut fat)?;
            assert_eq!(&fat, b"FAT32   ");
            println!("BUSY_PARTITION_RETAINS_WHOLE_SNAPSHOT_PASS");
            // Terminal failure retains owners until disposal of this guest.
            std::process::exit(0);
        }
        Self::contents(filesystem, root_mib, &partitions, None, Some("base"))?;
        let scute = Arc::new(
            partitions
                .into_snapshot()?
                .close()?
                .detach()?
                .seal_snapshot([0; 32])?,
        );
        origin.close()?;
        let parent = ReadOnlyChain::assemble(None, &[scute])?;
        let before = Self::digest(parent.id(), bytes)?;
        println!("PARTITIONED_ROOT_AND_ESP_FORMAT_PASS");
        // Keep a parent filesystem with the same UUID mounted while writing
        // its child. Mount selection must never alias the parent's device.
        let mut reader_store = workspace.scratch()?;
        reader_store.set_len(128 * 1024 * 1024)?;
        let reader_snapshot =
            Mapping::<Created>::create()?.snapshot(&parent, reader_store.attach()?, sectors)?;
        let reader_partitions = PartitionedSnapshot::open(reader_snapshot)?;
        fs::create_dir_all("/work/parent")?;
        Self::command(
            "mount",
            &[
                "-t",
                filesystem,
                "-o",
                if filesystem == "xfs" {
                    "ro,nouuid"
                } else {
                    "ro"
                },
                &Self::node(reader_partitions.root_id()),
                "/work/parent",
            ],
        )?;
        let mut scratch = workspace.scratch()?;
        scratch.set_len(128 * 1024 * 1024)?;
        let snapshot =
            Mapping::<Created>::create()?.snapshot(&parent, scratch.attach()?, sectors)?;
        let partitions = PartitionedSnapshot::open(snapshot)?;
        Self::contents(
            filesystem,
            root_mib,
            &partitions,
            Some("base"),
            Some("extended"),
        )?;
        let scute = Arc::new(
            partitions
                .into_snapshot()?
                .close()?
                .detach()?
                .seal_snapshot(*parent.root())?,
        );
        assert_eq!(Self::digest(parent.id(), bytes)?, before);
        println!("PARTITION_WRITES_LEAVE_PARENT_UNCHANGED_PASS");
        assert_eq!(fs::read_to_string("/work/parent/marker")?, "base");
        Self::command("umount", &["/work/parent"])?;
        reader_partitions.into_snapshot()?.close()?.detach()?;
        println!("CONCURRENT_PARENT_CHILD_{filesystem}_PASS");
        let extended = parent.extend(scute)?;
        let mut scratch = workspace.scratch()?;
        scratch.set_len(128 * 1024 * 1024)?;
        let snapshot =
            Mapping::<Created>::create()?.snapshot(&extended, scratch.attach()?, sectors)?;
        let partitions = PartitionedSnapshot::open(snapshot)?;
        Self::contents(filesystem, root_mib, &partitions, Some("extended"), None)?;
        let views = [partitions.root_id(), partitions.esp_id().unwrap()];
        let snapshot = partitions.into_snapshot()?;
        for view in views {
            assert!(!Path::new(&Self::node(view)).exists());
        }
        snapshot.close()?.detach()?;
        println!("PARTITION_VIEWS_RELEASE_BEFORE_SEALING_PASS");
        extended.close()?;
        // Carapace's shared immutable mappings use deferred destruction. Our
        // writable partition views above were removed synchronously.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while super::chain_storage::ChainStorage::mappings()? != baseline {
            assert!(
                std::time::Instant::now() < deadline,
                "shared chain mappings leaked"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        println!("WHOLE_DISK_LAYER_PRESERVES_BOTH_PARTITIONS_PASS");
        Ok(())
    }
}
