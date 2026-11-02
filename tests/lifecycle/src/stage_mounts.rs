// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    contract::recipe::Format,
    contract::recipe::{ImagePath, Mount},
    execution::HelperProgram,
    formatter::{CapturedFormatter, Ext4Format},
    step::SetupBudget,
    storage::{Active, Created, Mapping, Runtime, SnapshotMapping, Volume, Workspace},
};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io,
    num::NonZeroU64,
    os::{
        fd::AsRawFd,
        unix::fs::{PermissionsExt, symlink},
    },
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub(super) struct StageMounts;
impl StageMounts {
    fn format<'a, 'w>(
        origin: &'a Mapping<Active>,
        workspace: &'w Workspace,
        esp: bool,
    ) -> anyhow::Result<SnapshotMapping<'a, 'w>> {
        let mut scratch = workspace.scratch()?;
        scratch.set_len(32 * 1024 * 1024)?;
        let snapshot = Mapping::<Created>::create()?.snapshot(
            origin,
            scratch.attach()?,
            NonZeroU64::new(carapace::ZERO_SIZE_SECTORS).unwrap(),
        )?;
        let spec: Format = serde_saphyr::from_str(if esp {
            "kind: ext4\nsize: 32MiB\nesp: 257MiB"
        } else {
            "kind: ext4\nsize: 32MiB"
        })?;
        let (formatter, snapshot) = CapturedFormatter::for_filesystem(&spec)?
            .stage(workspace)?
            .format(
                snapshot,
                Ext4Format::from(&spec),
                &HelperProgram::current()?,
                SetupBudget::begin(),
            )?
            .wait()?
            .into_parts()?;
        formatter.close()?;
        Ok(snapshot)
    }

    fn mounts(pairs: &[(&str, Mount)]) -> BTreeMap<ImagePath, Mount> {
        pairs
            .iter()
            .map(|(path, kind)| (ImagePath::try_from((*path).to_owned()).unwrap(), *kind))
            .collect()
    }

    fn root(file: &File) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()))
    }

    pub(super) fn run(busy: bool) -> Result<(), Box<dyn std::error::Error>> {
        let workspace = Workspace::create()?;
        let origin = Mapping::<Created>::create()?
            .zero(NonZeroU64::new(carapace::ZERO_SIZE_SECTORS).unwrap())?;
        let parent = Path::new("/work/stage-runtime");
        fs::create_dir(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        let runtime = Runtime::create(&workspace, parent)?;
        let paths = runtime.paths()?;
        let mounts = Self::mounts(&[
            ("/boot/efi", Mount::Esp),
            ("/tmp", Mount::Tmpfs),
            ("/tmp/nested", Mount::Tmpfs),
        ]);
        let snapshot = Self::format(&origin, &workspace, true)?;
        let snapshot_id = snapshot.id();
        let volume = Volume::in_init_namespace(snapshot, runtime.recovery())?
            .with_mounts(&mounts, paths.volumes(), true)?
            .mount_ext4(paths.volumes())?;
        {
            let view = volume.view()?;
            let descriptor = view.descriptor()?;
            let root = Self::root(&descriptor);
            fs::write(root.join("boot/efi/marker"), b"persisted")?;
            fs::write(root.join("tmp/marker"), b"temporary")?;
            fs::write(root.join("tmp/nested/marker"), b"nested")?;
            fs::write(root.join("root-marker"), b"root")?;
        }
        if busy {
            // Hold a directory in the supervisor's actual child mount, not a
            // clone. A normal unmount must refuse to detach this live owner.
            let root = fs::read_dir(paths.volumes())?
                .map(|entry| entry.unwrap().path())
                .find(|path| path.join("root-marker").is_file())
                .unwrap();
            let holder = File::open(root.join("tmp/nested"))?;
            assert_eq!(
                volume.unmount().unwrap_err().kind(),
                io::ErrorKind::ResourceBusy
            );
            assert_eq!(fs::read(Self::root(&holder).join("marker"))?, b"nested");
            assert!(Path::new(&format!("/dev/dm-{}", snapshot_id.minor())).exists());
            drop(paths);
            assert!(runtime.close().is_err());
            println!("BUSY_STAGE_MOUNT_RETAINS_SNAPSHOT_AND_RUNTIME_PASS");
            std::process::exit(0);
        }
        let volume = volume.unmount()?.mount_ext4(paths.volumes())?;
        {
            let view = volume.view()?;
            let descriptor = view.descriptor()?;
            let root = Self::root(&descriptor);
            assert_eq!(fs::read(root.join("tmp/marker"))?, b"temporary");
            assert_eq!(fs::read(root.join("tmp/nested/marker"))?, b"nested");
            assert_eq!(fs::read(root.join("boot/efi/marker"))?, b"persisted");
        }
        let snapshot = volume.unmount()?.into_snapshot()?;
        assert_eq!(fs::read_dir(paths.volumes())?.count(), 0);
        let volume =
            Volume::in_init_namespace(snapshot, runtime.recovery())?.mount_ext4(paths.volumes())?;
        {
            let view = volume.view()?;
            let descriptor = view.descriptor()?;
            let root = Self::root(&descriptor);
            assert!(!root.join("tmp/marker").exists());
            assert!(!root.join("tmp/nested/marker").exists());
            assert!(!root.join("boot/efi/marker").exists());
            assert_eq!(fs::read(root.join("root-marker"))?, b"root");
        }
        let volume = volume
            .unmount()?
            .with_mounts(
                &Self::mounts(&[("boot/efi", Mount::Esp)]),
                paths.volumes(),
                false,
            )?
            .mount_ext4(paths.volumes())?;
        {
            let view = volume.view()?;
            let descriptor = view.descriptor()?;
            assert_eq!(
                fs::read(Self::root(&descriptor).join("boot/efi/marker"))?,
                b"persisted"
            );
        }
        volume.unmount()?.into_snapshot()?.close()?.detach()?;
        println!("STAGE_MOUNTS_SURVIVE_DIRECTIVES_NOT_STAGE_PASS");

        // Missing ESP occurs after a temporary master was already acquired.
        let snapshot = Self::format(&origin, &workspace, false)?;
        let error = Volume::in_init_namespace(snapshot, runtime.recovery())?
            .with_mounts(
                &Self::mounts(&[("a", Mount::Tmpfs), ("b", Mount::Esp)]),
                paths.volumes(),
                false,
            )
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(fs::read_dir(paths.volumes())?.count(), 0);
        println!("MISSING_ESP_RELEASES_PARTIAL_STAGE_PASS");

        // Reject a symlink after attaching the first child. Roll back children,
        // root, masters and partition views through explicit close paths.
        let snapshot = Self::format(&origin, &workspace, false)?;
        let volume =
            Volume::in_init_namespace(snapshot, runtime.recovery())?.mount_ext4(paths.volumes())?;
        {
            let view = volume.view()?;
            let descriptor = view.descriptor()?;
            symlink("/outside", Self::root(&descriptor).join("bad"))?;
        }
        let volume = volume.unmount()?.with_mounts(
            &Self::mounts(&[("a", Mount::Tmpfs), ("bad", Mount::Tmpfs)]),
            paths.volumes(),
            false,
        )?;
        let error = volume.mount_ext4(paths.volumes()).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(40));
        assert_eq!(fs::read_dir(paths.volumes())?.count(), 0);
        println!("SYMLINK_MOUNT_RELEASES_PARTIAL_VIEWS_PASS");
        drop(paths);
        runtime.close()?;
        origin.close()?;
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        Ok(())
    }
}
