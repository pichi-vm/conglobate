// SPDX-License-Identifier: Apache-2.0
//! Benign mount compatibility probe, not the production workspace owner.
//! Any failed assertion ends the disposable guest; no host mounts are touched.
use rustix::{fs as rfs, mount};
use std::{fs, fs::File, io, os::fd::AsRawFd, os::unix::fs::PermissionsExt, path::Path};

#[derive(Debug)]
pub(super) struct WorkspaceMount;
impl WorkspaceMount {
    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        let context = mount::fsopen("tmpfs", mount::FsOpenFlags::FSOPEN_CLOEXEC)?;
        mount::fsconfig_set_string(&context, "mode", "0700")?;
        mount::fsconfig_create(&context)?;
        let root = mount::fsmount(
            &context,
            mount::FsMountFlags::FSMOUNT_CLOEXEC,
            mount::MountAttrFlags::MOUNT_ATTR_NODEV
                | mount::MountAttrFlags::MOUNT_ATTR_NOSUID
                | mount::MountAttrFlags::MOUNT_ATTR_NOEXEC,
        )?;
        let via_fd = format!("/proc/self/fd/{}", root.as_raw_fd());
        fs::create_dir(format!("{via_fd}/context"))?;
        fs::write(format!("{via_fd}/context/sentinel"), b"private checkout")?;
        fs::write(format!("{via_fd}/resolver"), b"nameserver 192.0.2.1\n")?;
        let flags = mount::OpenTreeFlags::OPEN_TREE_CLONE | mount::OpenTreeFlags::OPEN_TREE_CLOEXEC;
        match mount::open_tree(&root, "context", flags) {
            Ok(view) => {
                drop(view);
                println!("DETACHED_SOURCE_SUPPORTED");
            }
            Err(rustix::io::Errno::INVAL) => println!("DETACHED_SOURCE_REJECTED_EINVAL"),
            Err(error) => return Err(error.into()),
        }
        let target = Path::new("/work/attached-workspace");
        fs::create_dir(target)?;
        fs::set_permissions(target, fs::Permissions::from_mode(0o700))?;
        mount::mount_setattr(
            &root,
            "",
            mount::MountSetAttrFlags::AT_EMPTY_PATH,
            mount::MountAttr::new(
                mount::MountAttrFlags::empty(),
                mount::MountAttrFlags::empty(),
                mount::MountPropagationFlags::PRIVATE,
            ),
        )?;
        mount::move_mount(
            &root,
            "",
            rustix::fs::CWD,
            target,
            mount::MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
        )?;
        assert_eq!(rfs::fstatfs(&root)?.f_type as u64, 0x0102_1994);
        for (name, child, bytes) in [
            ("context", "sentinel", b"private checkout".as_slice()),
            ("resolver", "", b"nameserver 192.0.2.1\n".as_slice()),
        ] {
            let view = mount::open_tree(&root, name, flags)?;
            mount::mount_setattr(
                &view,
                "",
                mount::MountSetAttrFlags::AT_EMPTY_PATH,
                mount::MountAttr::new(
                    mount::MountAttrFlags::MOUNT_ATTR_RDONLY,
                    mount::MountAttrFlags::empty(),
                    mount::MountPropagationFlags::PRIVATE,
                ),
            )?;
            let path = format!(
                "/proc/self/fd/{}{}{}",
                view.as_raw_fd(),
                if child.is_empty() { "" } else { "/" },
                child
            );
            assert_eq!(fs::read(&path)?, bytes);
            assert_eq!(
                File::options()
                    .write(true)
                    .open(&path)
                    .unwrap_err()
                    .raw_os_error(),
                Some(rustix::io::Errno::ROFS.raw_os_error())
            );
        }
        println!("ATTACHED_READONLY_DIRECTORY_AND_FILE_PASS");
        // An ordinary live directory FD must make normal unmount fail; after
        // releasing it, explicit unmount must succeed without lazy detachment.
        assert_eq!(
            mount::unmount(target, mount::UnmountFlags::empty()).unwrap_err(),
            rustix::io::Errno::BUSY
        );
        drop(root);
        mount::unmount(target, mount::UnmountFlags::empty())?;
        if fs::read_dir(target)?.next().is_some() {
            return Err(io::Error::other("workspace backing exposed after unmount").into());
        }
        fs::remove_dir(target)?;
        println!("ATTACHED_WORKSPACE_NORMAL_UNMOUNT_PASS");
        Ok(())
    }
}
