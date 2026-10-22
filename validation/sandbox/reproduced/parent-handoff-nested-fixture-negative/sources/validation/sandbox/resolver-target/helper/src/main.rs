#![forbid(unsafe_code)]

use nix::mount::{MntFlags, MsFlags, mount, umount, umount2};
use rustix::fs::{FileType, Mode, OFlags, ResolveFlags, fstat, open, openat2};
use rustix::io::Errno;
use std::{
    fs,
    io::Read,
    os::{
        fd::{AsRawFd, OwnedFd},
        unix::{fs::symlink, net::UnixListener},
    },
    path::Path,
};

fn select(root: &OwnedFd, path: &str) -> Result<OwnedFd, Errno> {
    let fd = openat2(
        root,
        path,
        OFlags::PATH | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::IN_ROOT | ResolveFlags::NO_MAGICLINKS | ResolveFlags::NO_XDEV,
    )?;
    if FileType::from_raw_mode(fstat(&fd)?.st_mode) != FileType::RegularFile {
        return Err(Errno::INVAL);
    }
    Ok(fd)
}

fn fd_path(fd: &OwnedFd) -> String {
    format!("/proc/self/fd/{}", fd.as_raw_fd())
}

fn cancellation_gate(stage: &str) {
    if std::env::var("RESOLVER_TEST_GATE").as_deref() != Ok(stage) {
        return;
    }
    // Test-only handoff through ancestor proc before installing private proc.
    // The trusted controller pins this PID while this bounded gate is held.
    let status = fs::read_to_string("/proc/self/status").unwrap();
    let pid = status
        .lines()
        .find_map(|line| line.strip_prefix("NSpid:"))
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap();
    fs::write("/work/resolver-ready.tmp", format!("{stage} {pid}\n")).unwrap();
    fs::rename("/work/resolver-ready.tmp", "/work/resolver-ready").unwrap();
    std::thread::sleep(std::time::Duration::from_secs(10));
    panic!("resolver cancellation gate expired: {stage}");
}

fn read(root: &OwnedFd, path: &str) -> Vec<u8> {
    let fd = openat2(
        root,
        path,
        OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::IN_ROOT | ResolveFlags::NO_MAGICLINKS,
    )
    .unwrap();
    let mut bytes = Vec::new();
    fs::File::from(fd).read_to_end(&mut bytes).unwrap();
    bytes
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "--reject") {
        assert_eq!(args.len(), 5);
        let root = open(
            args[2].as_str(),
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();
        let expected: i32 = args[4].parse().unwrap();
        assert_eq!(
            select(&root, &args[3]).unwrap_err().raw_os_error(),
            expected
        );
        println!("RESOLVER_FUSE_REJECT_PASS {}", args[3]);
        return;
    }
    if args.get(1).is_some_and(|arg| arg == "--install") {
        use std::os::unix::process::CommandExt;
        let separator = args.iter().position(|arg| arg == "--").unwrap();
        assert!(separator > 4 && args.len() > separator + 1);
        let root = open(
            args[2].as_str(),
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();
        let source = open(
            args[3].as_str(),
            OFlags::PATH | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();
        assert_eq!(
            FileType::from_raw_mode(fstat(&source).unwrap().st_mode),
            FileType::RegularFile
        );
        // Select every target before installing mounts; NO_XDEV would reject
        // a later path that aliases an already mounted target.
        let targets: Vec<_> = args[4..separator]
            .iter()
            .map(|path| select(&root, path).unwrap())
            .collect();
        cancellation_gate("before-bind");
        for (index, (path, target)) in args[4..separator].iter().zip(&targets).enumerate() {
            mount(
                Some(fd_path(&source).as_str()),
                fd_path(target).as_str(),
                None::<&str>,
                MsFlags::MS_BIND,
                None::<&str>,
            )
            .unwrap();
            if index == 0 {
                cancellation_gate("after-bind");
            }
            let covering = openat2(
                &root,
                path.as_str(),
                OFlags::PATH | OFlags::CLOEXEC,
                Mode::empty(),
                ResolveFlags::IN_ROOT | ResolveFlags::NO_MAGICLINKS,
            )
            .unwrap();
            mount(
                None::<&str>,
                fd_path(&covering).as_str(),
                None::<&str>,
                MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY,
                None::<&str>,
            )
            .unwrap();
            if index == 0 {
                cancellation_gate("after-readonly");
            }
        }
        drop(targets);
        drop(source);
        drop(root);
        println!("RESOLVER_FUSE_DESCRIPTOR_INSTALL_PASS");
        // Trusted continuation sets runtime mounts and invokes the entry
        // helper. This helper creates no child that can retain the namespace.
        panic!(
            "trusted continuation exec failed: {}",
            std::process::Command::new(&args[separator + 1])
                .args(&args[separator + 2..])
                .exec()
        );
    }
    assert_eq!(args.len(), 1);
    let tree = Path::new("/work/root");
    for name in ["etc", "run", "nested"] {
        fs::create_dir_all(tree.join(name)).unwrap();
    }
    fs::write(tree.join("run/resolver"), b"original image resolver\n").unwrap();
    fs::write(tree.join("etc/regular"), b"original image resolver\n").unwrap();
    fs::write("/work/resolver", b"nameserver 192.0.2.53\n").unwrap();
    fs::write("/work/outside-canary", b"outside unchanged\n").unwrap();
    for (name, target) in [
        ("absolute", "/run/resolver"),
        ("relative", "../run/resolver"),
        ("clamped", "../../../../run/resolver"),
        ("dangling", "/run/missing"),
        ("cycle", "cycle"),
        ("outside", "/work/outside-canary"),
        ("file-slash", "regular/"),
        ("file-dot", "regular/."),
    ] {
        symlink(target, tree.join("etc").join(name)).unwrap();
    }
    nix::unistd::mkfifo(&tree.join("fifo"), nix::sys::stat::Mode::S_IRUSR).unwrap();
    nix::sys::stat::mknod(
        &tree.join("device"),
        nix::sys::stat::SFlag::S_IFCHR,
        nix::sys::stat::Mode::S_IRUSR,
        nix::sys::stat::makedev(1, 3),
    )
    .unwrap();
    let _socket = UnixListener::bind(tree.join("socket")).unwrap();
    let root = open(
        tree,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .unwrap();
    for (path, expected) in [
        ("/etc/dangling", Errno::NOENT),
        ("/etc/cycle", Errno::LOOP),
        ("/etc/outside", Errno::NOENT),
        ("/missing/../etc/regular", Errno::NOENT),
        ("/etc/regular/", Errno::NOTDIR),
        ("/etc/file-slash", Errno::NOTDIR),
        ("/etc/file-dot", Errno::NOTDIR),
        ("/fifo", Errno::INVAL),
        ("/socket", Errno::INVAL),
        ("/device", Errno::INVAL),
        ("/run", Errno::INVAL),
    ] {
        assert_eq!(select(&root, path).unwrap_err(), expected, "{path}");
        println!("RESOLVER_TARGET_REJECT_PASS {path}");
    }
    // No file content open is used before regular-file validation: FIFO and
    // device probes cannot invoke their read/write operations.
    mount(
        Some("/work"),
        &tree.join("nested"),
        None::<&str>,
        MsFlags::MS_BIND,
        None::<&str>,
    )
    .unwrap();
    assert_eq!(select(&root, "/nested/resolver").unwrap_err(), Errno::XDEV);
    umount(&tree.join("nested")).unwrap();
    let proc = open("/proc", OFlags::PATH | OFlags::DIRECTORY, Mode::empty()).unwrap();
    let source = open(
        "/work/resolver",
        OFlags::PATH | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .unwrap();
    assert_eq!(
        select(&proc, &format!("self/fd/{}", source.as_raw_fd())).unwrap_err(),
        Errno::LOOP
    );
    println!("RESOLVER_TARGET_MOUNT_AND_MAGICLINK_DENIALS_PASS");
    for path in [
        "/etc/regular",
        "/etc/absolute",
        "/etc/relative",
        "/etc/clamped",
    ] {
        let target = select(&root, path).unwrap();
        let before = fstat(&target).unwrap();
        mount(
            Some(fd_path(&source).as_str()),
            fd_path(&target).as_str(),
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>,
        )
        .unwrap();
        // Reopen through the root to hold the covering mount for remount and
        // unmount. The pre-mount handle still identifies the image inode.
        let covering = openat2(
            &root,
            path,
            OFlags::PATH | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::IN_ROOT | ResolveFlags::NO_MAGICLINKS,
        )
        .unwrap();
        mount(
            None::<&str>,
            fd_path(&covering).as_str(),
            None::<&str>,
            MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY,
            None::<&str>,
        )
        .unwrap();
        assert_eq!(read(&root, path), b"nameserver 192.0.2.53\n");
        assert_eq!(
            openat2(
                &root,
                path,
                OFlags::WRONLY,
                Mode::empty(),
                ResolveFlags::IN_ROOT | ResolveFlags::NO_MAGICLINKS
            )
            .unwrap_err(),
            Errno::ROFS
        );
        // The covering O_PATH handle pins this mount. Detach it from this
        // private tree, then release the final handle before checking restore.
        umount2(fd_path(&covering).as_str(), MntFlags::MNT_DETACH).unwrap();
        drop(covering);
        assert_eq!(read(&root, path), b"original image resolver\n");
        let after = fstat(select(&root, path).unwrap()).unwrap();
        assert_eq!(
            (
                before.st_dev,
                before.st_ino,
                before.st_mode,
                before.st_uid,
                before.st_gid
            ),
            (
                after.st_dev,
                after.st_ino,
                after.st_mode,
                after.st_uid,
                after.st_gid
            )
        );
        println!("RESOLVER_TARGET_BIND_RESTORE_PASS {path}");
    }
    assert_eq!(
        fs::read("/work/outside-canary").unwrap(),
        b"outside unchanged\n"
    );
    assert_eq!(
        fs::read("/work/resolver").unwrap(),
        b"nameserver 192.0.2.53\n"
    );
    assert_eq!(
        fs::read_link(tree.join("etc/absolute")).unwrap(),
        Path::new("/run/resolver")
    );
    assert!(!tree.join("run/missing").exists());
    println!("RESOLVER_TARGET_MATRIX_PASS");
}
