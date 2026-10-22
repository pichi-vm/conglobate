//! Calls the real COPY implementation from a trusted worker-side process.
#![forbid(unsafe_code)]
use conglobate::recipe::{Copy, CopySources, GuestPath};
use std::{
    error::Error,
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    path::Path,
};

fn operation(source: &str, destination: &str) -> Copy {
    Copy {
        from: CopySources::One(source.into()),
        into: GuestPath::try_from(destination.to_owned()).unwrap(),
        owner: None,
        group: None,
        mode: None,
    }
}

fn fixture_acl() -> Vec<u8> {
    let mut acl = 2u32.to_le_bytes().to_vec();
    for (tag, permissions, identity) in [
        (1u16, 7u16, u32::MAX),
        (2, 4, 4321),
        (4, 5, u32::MAX),
        (16, 5, u32::MAX),
        (32, 1, u32::MAX),
    ] {
        acl.extend(tag.to_le_bytes());
        acl.extend(permissions.to_le_bytes());
        acl.extend(identity.to_le_bytes());
    }
    acl
}

fn main() -> Result<(), Box<dyn Error>> {
    let mode = std::env::args().nth(1).ok_or("missing mode")?;
    let context = Path::new("/work/context");
    let root = Path::new("/work/view");
    match mode.as_str() {
        "prepare" => {
            fs::write(context.join("file"), "context fixture\n")?;
            fs::set_permissions(context.join("file"), fs::Permissions::from_mode(0o751))?;
            xattr::set(context.join("file"), "user.fixture", b"preserved")?;
            xattr::set(
                context.join("file"),
                "system.posix_acl_access",
                &fixture_acl(),
            )?;
            xattr::set(
                context.join("file"),
                "security.selinux",
                b"system_u:object_r:conglobate_target_only_t:s0",
            )?;
            let mut capability = 0x0200_0001u32.to_le_bytes().to_vec();
            for bits in [1u32 << 10, 0, 0, 0] {
                capability.extend(bits.to_le_bytes());
            }
            xattr::set(context.join("file"), "security.capability", &capability)?;
            filetime::set_file_times(
                context.join("file"),
                filetime::FileTime::from_unix_time(1_700_000_000, 123_000_000),
                filetime::FileTime::from_unix_time(1_700_000_001, 456_000_000),
            )?;
            fs::create_dir(context.join("directory"))?;
            fs::write(context.join("directory/child"), "child\n")?;
            symlink("child", context.join("directory/link"))?;
            symlink("/file", context.join("source-link"))?;
            println!("COPY_CONTEXT_METADATA_PREPARED");
        }
        "copy" => {
            // This process is invoked only after command PID-namespace teardown.
            let error = fs::write(context.join("must-not-create"), b"fixture").unwrap_err();
            assert_eq!(error.raw_os_error(), Some(nix::libc::EROFS));
            let mut named = operation("source-link", "/destination/owned");
            named.owner = Some("builder".into());
            named.apply(context, root, None)?;
            let target = root.join("safe/owned");
            assert_eq!(fs::read_to_string(&target)?, "context fixture\n");
            let metadata = fs::metadata(&target)?;
            assert_eq!((metadata.uid(), metadata.gid()), (1234, 2345));
            assert_eq!(metadata.mode() & 0o777, 0o751);
            assert_eq!(metadata.mtime(), 1_700_000_001);
            assert_eq!(metadata.mtime_nsec(), 456_000_000);
            for name in [
                "user.fixture",
                "security.selinux",
                "security.capability",
                "system.posix_acl_access",
            ] {
                assert_eq!(
                    xattr::get(&target, name)?,
                    xattr::get(context.join("file"), name)?,
                    "{name}",
                );
            }
            operation("file", "/outside-link").apply(context, root, None)?;
            assert_eq!(
                fs::read_to_string("/outside-canary")?,
                "supervisor fixture\n"
            );
            assert_eq!(
                fs::read_to_string(root.join("outside-canary"))?,
                "context fixture\n"
            );
            operation("file", "/appended/").apply(context, root, None)?;
            assert!(fs::symlink_metadata(root.join("appended/file"))?.is_file());
            assert_eq!(
                fs::read_to_string(root.join("image-canary"))?,
                "image fixture\n"
            );
            operation("directory", "/copied").apply(context, root, None)?;
            assert_eq!(fs::read_link(root.join("copied/link"))?, Path::new("child"));
            assert!(
                operation("file", "/cycle")
                    .apply(context, root, None)
                    .is_err()
            );
            assert!(
                operation("file", "/$CARAPACE")
                    .apply(context, root, None)
                    .is_err()
            );
            println!("COPY_REAL_IMPLEMENTATION_METADATA_AND_PATHS_PASS");
        }
        "fifo-account" => {
            fs::remove_file(root.join("etc/passwd"))?;
            nix::unistd::mkfifo(&root.join("etc/passwd"), nix::sys::stat::Mode::S_IRUSR)?;
            let mut named = operation("file", "/should-not-be-produced");
            named.owner = Some("builder".into());
            println!("COPY_FIFO_ACCOUNT_READ_BEGIN");
            // The trusted outer timeout also bounds this regression assertion.
            let error = named.apply(context, root, None).unwrap_err();
            assert!(error.to_string().contains("must be a regular file"));
            println!("COPY_FIFO_ACCOUNT_REJECTED_PASS");
        }
        "disk" => {
            let target = Path::new("/work/remount/safe/owned");
            let metadata = fs::metadata(target)?;
            assert_eq!((metadata.uid(), metadata.gid()), (1234, 2345));
            assert_eq!(metadata.mode() & 0o777, 0o751);
            assert_eq!(metadata.mtime(), 1_700_000_001);
            assert_eq!(metadata.mtime_nsec(), 456_000_000);
            assert_eq!(
                xattr::get(target, "security.selinux")?.unwrap(),
                b"system_u:object_r:conglobate_target_only_t:s0"
            );
            let mut capability = 0x0200_0001u32.to_le_bytes().to_vec();
            for bits in [1u32 << 10, 0, 0, 0] {
                capability.extend(bits.to_le_bytes());
            }
            assert_eq!(
                xattr::get(target, "security.capability")?.unwrap(),
                capability
            );
            assert_eq!(xattr::get(target, "user.fixture")?.unwrap(), b"preserved");
            assert_eq!(
                xattr::get(target, "system.posix_acl_access")?.unwrap(),
                fixture_acl()
            );
            println!("COPY_KERNEL_REMOUNT_METADATA_PASS");
        }
        _ => return Err("unknown mode".into()),
    }
    Ok(())
}
