// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::{
    paths::resolve,
    recipe::{Copy, CopySources, GuestPath},
};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    path::Path,
};

fn copy(from: &str, into: &str) -> Copy {
    Copy {
        from: CopySources::One(from.into()),
        into: GuestPath::try_from(into.to_owned()).unwrap(),
        owner: None,
        group: None,
        mode: None,
    }
}

#[test]
fn image_paths_never_resolve_absolute_links_against_the_host() {
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir(directory.path().join("real")).unwrap();
    symlink("/real", directory.path().join("link")).unwrap();
    assert_eq!(
        resolve(directory.path(), Path::new("/link/file"), true).unwrap(),
        directory.path().join("real/file")
    );
    symlink("../../../../real", directory.path().join("relative")).unwrap();
    assert_eq!(
        resolve(directory.path(), Path::new("/relative/file"), true).unwrap(),
        directory.path().join("real/file")
    );
    symlink("/cycle", directory.path().join("cycle")).unwrap();
    assert!(resolve(directory.path(), Path::new("/cycle"), true).is_err());
}

#[test]
#[ignore = "requires root for Docker COPY ownership semantics"]
fn copy_contents_links_patterns_metadata_and_ownership() {
    let directory = tempfile::tempdir().unwrap();
    let context = directory.path().join("context");
    let root = directory.path().join("root");
    fs::create_dir_all(context.join("source")).unwrap();
    fs::create_dir_all(root.join("etc")).unwrap();
    fs::write(context.join("source/file"), "content").unwrap();
    fs::set_permissions(
        context.join("source/file"),
        fs::Permissions::from_mode(0o751),
    )
    .unwrap();
    xattr::set(context.join("source/file"), "user.fixture", b"preserved").unwrap();
    symlink("file", context.join("source/nested-link")).unwrap();
    symlink("source/file", context.join("top-link")).unwrap();
    fs::write(root.join("etc/passwd"), "builder:x:1234:9876::/:/bin/sh\n").unwrap();
    fs::write(root.join("etc/group"), "builders:x:2345:\n").unwrap();
    copy("source", "/dest")
        .apply(&context, &root, None)
        .unwrap();
    assert_eq!(
        fs::read_to_string(root.join("dest/file")).unwrap(),
        "content"
    );
    assert_eq!(
        fs::read_link(root.join("dest/nested-link")).unwrap(),
        Path::new("file")
    );
    assert_eq!(
        fs::metadata(root.join("dest/file")).unwrap().mode() & 0o777,
        0o751
    );
    assert_eq!(
        xattr::get(root.join("dest/file"), "user.fixture")
            .unwrap()
            .unwrap(),
        b"preserved"
    );
    copy("top-link", "/followed")
        .apply(&context, &root, None)
        .unwrap();
    assert!(
        fs::symlink_metadata(root.join("followed"))
            .unwrap()
            .is_file()
    );
    symlink("/dest", root.join("absolute")).unwrap();
    copy("top-link", "/absolute/inside")
        .apply(&context, &root, None)
        .unwrap();
    assert_eq!(
        fs::read_to_string(root.join("dest/inside")).unwrap(),
        "content"
    );
    let mut configured = copy("source/f*", "/owned/");
    configured.owner = Some("builder".into());
    configured.group = Some("builders".into());
    configured.mode = Some("u=rw,go=r".into());
    configured.apply(&context, &root, None).unwrap();
    let metadata = fs::metadata(root.join("owned/file")).unwrap();
    assert_eq!(
        (metadata.uid(), metadata.gid(), metadata.mode() & 0o777),
        (1234, 2345, 0o644)
    );
    assert!(
        copy("nothing*", "/missing/")
            .apply(&context, &root, None)
            .is_err()
    );
    assert!(
        copy("source/*", "/not-a-directory")
            .apply(&context, &root, None)
            .is_err()
    );
    assert!(
        copy("file", "/$CARAPACE")
            .apply(&context, &root, None)
            .is_err()
    );
    copy("top-link", "/$CARAPACE")
        .apply(&context, &root, Some("bound"))
        .unwrap();
    assert!(root.join("bound").is_file());
    fs::create_dir(context.join("walk")).unwrap();
    fs::create_dir(context.join("walk/directory")).unwrap();
    fs::write(context.join("walk/directory/matched"), "matched").unwrap();
    fs::write(context.join("walk/file"), "not a directory").unwrap();
    symlink("directory", context.join("walk/link")).unwrap();
    copy("walk/*/matched", "/walk-result/")
        .apply(&context, &root, None)
        .unwrap();
    assert_eq!(
        fs::read(root.join("walk-result/matched")).unwrap(),
        b"matched"
    );
    symlink("/walk/directory", context.join("absolute-source")).unwrap();
    copy("absolute-source/*", "/absolute-result/")
        .apply(&context, &root, None)
        .unwrap();
    assert_eq!(
        fs::read(root.join("absolute-result/matched")).unwrap(),
        b"matched"
    );
}

#[test]
#[ignore = "requires root and filesystem support for ACLs and capabilities"]
fn copy_preserves_security_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let context = directory.path().join("context");
    let root = directory.path().join("root");
    fs::create_dir(&context).unwrap();
    fs::create_dir(&root).unwrap();
    let source = context.join("executable");
    fs::write(&source, b"fixture").unwrap();
    // Linux POSIX ACL xattr: version, then tag/permissions/identity entries.
    let mut acl = 2u32.to_le_bytes().to_vec();
    for (tag, permissions, identity) in [
        (1u16, 7u16, u32::MAX),
        (2, 4, 1234),
        (4, 5, u32::MAX),
        (16, 5, u32::MAX),
        (32, 0, u32::MAX),
    ] {
        acl.extend(tag.to_le_bytes());
        acl.extend(permissions.to_le_bytes());
        acl.extend(identity.to_le_bytes());
    }
    xattr::set(&source, "system.posix_acl_access", &acl).unwrap();
    // Revision 2 file capabilities, effective CAP_NET_BIND_SERVICE.
    let mut capability = 0x0200_0001u32.to_le_bytes().to_vec();
    for bits in [1u32 << 10, 0, 0, 0] {
        capability.extend(bits.to_le_bytes());
    }
    xattr::set(&source, "security.capability", &capability).unwrap();
    copy("executable", "/executable")
        .apply(&context, &root, None)
        .unwrap();
    for name in [
        "system.posix_acl_access",
        "security.capability",
        "security.selinux",
    ] {
        let original = xattr::get(&source, name).unwrap();
        if name != "security.selinux" || Path::new("/sys/fs/selinux/enforce").exists() {
            assert!(original.is_some(), "missing test metadata: {name}");
        }
        assert_eq!(
            xattr::get(root.join("executable"), name).unwrap(),
            original,
            "{name}"
        );
    }
}

#[test]
#[ignore = "requires root for Docker COPY ownership semantics"]
fn copy_replaces_appended_symlinks_and_preserves_destination_directory_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let context = directory.path().join("context");
    let root = directory.path().join("root");
    fs::create_dir_all(context.join("source")).unwrap();
    fs::create_dir_all(root.join("dest")).unwrap();
    fs::write(context.join("file"), "new").unwrap();
    fs::write(root.join("marker"), "old").unwrap();
    symlink("/marker", root.join("dest/file")).unwrap();
    copy("file", "/dest/").apply(&context, &root, None).unwrap();
    assert!(
        fs::symlink_metadata(root.join("dest/file"))
            .unwrap()
            .is_file()
    );
    assert_eq!(fs::read_to_string(root.join("marker")).unwrap(), "old");
    assert_eq!(fs::read_to_string(root.join("dest/file")).unwrap(), "new");
    fs::set_permissions(context.join("source"), fs::Permissions::from_mode(0o711)).unwrap();
    xattr::set(context.join("source"), "user.source", b"source only").unwrap();
    fs::write(context.join("source/child"), "child").unwrap();
    for into in ["/a", "/b/"] {
        copy("source", into).apply(&context, &root, None).unwrap();
        let dest = root.join(into.trim_matches('/'));
        assert_eq!(fs::metadata(&dest).unwrap().mode() & 0o7777, 0o755);
        assert_eq!(xattr::get(&dest, "user.source").unwrap(), None);
        assert_eq!(fs::read(dest.join("child")).unwrap(), b"child");
    }
    fs::set_permissions(root.join("a"), fs::Permissions::from_mode(0o700)).unwrap();
    copy("source", "/a").apply(&context, &root, None).unwrap();
    assert_eq!(fs::metadata(root.join("a")).unwrap().mode() & 0o7777, 0o700);
    assert!(
        copy("source", "/marker")
            .apply(&context, &root, None)
            .is_err()
    );
    // Named-owner resolution is also exercised through real chown.
    fs::create_dir(root.join("etc")).unwrap();
    fs::write(root.join("etc/passwd"), "builder:x:1234:9876::/:/bin/sh\n").unwrap();
    let mut owned = copy("file", "/owned");
    owned.owner = Some("builder".into());
    owned.apply(&context, &root, None).unwrap();
    let meta = fs::metadata(root.join("owned")).unwrap();
    assert_eq!((meta.uid(), meta.gid()), (1234, 9876));
}

#[test]
fn image_path_rejects_parent_traversal_through_a_regular_file() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("file"), b"not a directory").unwrap();
    fs::write(
        directory.path().join("resolver"),
        b"nameserver 192.0.2.53\n",
    )
    .unwrap();
    symlink("file/../resolver", directory.path().join("link")).unwrap();
    for path in ["/file/../resolver", "/link"] {
        let kernel_error =
            fs::metadata(directory.path().join(path.trim_start_matches('/'))).unwrap_err();
        assert_eq!(kernel_error.kind(), std::io::ErrorKind::NotADirectory);
        let error = resolve(directory.path(), Path::new(path), true).unwrap_err();
        assert!(
            matches!(error, conglobate::Error::Io(ref error)
            if error.kind() == std::io::ErrorKind::NotADirectory),
            "{error}"
        );
    }
}
