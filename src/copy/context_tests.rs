// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    commit::CommitIdentity,
    context::{CapturedContext, tests::Repository},
};
use std::path::Path;

#[test]
#[ignore = "requires root to assign image ownership"]
fn used_modules_resolve_recorded_objects_and_context_subdirectories() {
    use crate::path::ContextPath;
    for format in ["sha1", "sha256"] {
        let child = Repository::new(format);
        child.write("dir/file", b"module bytes");
        child.commit();
        let repo = Repository::new(format);
        repo.module(&child, "vendor/lib", "different-name");
        let commit = repo.commit();
        // Neither local config nor working .gitmodules controls resolution.
        repo.write(".gitmodules", b"invalid working metadata");
        child.write("dir/file", b"later bytes");
        child.commit();
        let mut context = repo.capture(&commit, ".").unwrap();
        let path = ContextPath::try_from(Path::new("vendor/lib/dir/file")).unwrap();
        assert_eq!(context.read(&path).unwrap().bytes(), b"module bytes");
        crate::copy::CopySources::prepare(&mut context, &["vendor/lib".parse().unwrap()]).unwrap();
        let output = tempfile::tempdir().unwrap();
        let root = std::fs::File::open(output.path()).unwrap();
        let directive = crate::planner::Copy::fixture("vendor/lib", "/copied");
        crate::copy::CopyDestination::new(&root)
            .copy(&context, &directive)
            .unwrap();
        assert_eq!(
            std::fs::read(output.path().join("copied/dir/file")).unwrap(),
            b"module bytes"
        );
        let mut context = repo.capture(&commit, "vendor/lib/dir").unwrap();
        let path = ContextPath::try_from(Path::new("file")).unwrap();
        assert_eq!(context.read(&path).unwrap().bytes(), b"module bytes");
    }
}

#[test]
fn copy_preparation_requires_only_selected_module_stores() {
    use crate::copy::{CopySources, SourcePattern};
    let child = Repository::new("sha1");
    child.write("file", b"module");
    child.commit();
    let repo = Repository::new("sha1");
    repo.write("kept", b"ordinary");
    repo.module(&child, "used", "used-store");
    repo.module(&child, "unused", "unused-store");
    let commit = repo.commit();
    std::fs::remove_dir_all(repo.0.path().join(".git/modules/unused-store")).unwrap();
    let prepare = |context: &mut CapturedContext, pattern: &str| {
        CopySources::prepare(context, &[pattern.parse::<SourcePattern>().unwrap()])
    };
    let mut context = repo.capture(&commit, ".").unwrap();
    prepare(&mut context, "kept").unwrap();
    prepare(&mut context, "us*/file").unwrap();
    let error = prepare(&mut context, "unused/file")
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("unused") && error.contains("needed store"),
        "{error}"
    );
    for pattern in ["unused", "/", "un*"] {
        let mut context = repo.capture(&commit, ".").unwrap();
        assert!(prepare(&mut context, pattern).is_err(), "{pattern}");
    }
}

#[test]
fn nested_module_depth_is_limited_to_eight_used_gitlinks() {
    use crate::{copy::CopySources, path::ContextPath};
    let mut child = Repository::new("sha1");
    child.write("file", b"deep bytes");
    child.commit();
    for _ in 0..9 {
        let parent = Repository::new("sha1");
        parent.write("file", b"at this depth");
        parent.module(&child, "nested", "store");
        parent.commit();
        child = parent;
    }
    let commit = CommitIdentity::try_from(child.git(&["rev-parse", "HEAD"]).as_str()).unwrap();
    let mut context = child.capture(&commit, ".").unwrap();
    let within = format!("{}file", "nested/".repeat(8));
    let path = ContextPath::try_from(Path::new(&within)).unwrap();
    assert_eq!(context.read(&path).unwrap().bytes(), b"at this depth");
    CopySources::prepare(&mut context, &[within.parse().unwrap()]).unwrap();
    let beyond = format!("{}file", "nested/".repeat(9));
    let path = ContextPath::try_from(Path::new(&beyond)).unwrap();
    let error = context.read(&path).unwrap_err().to_string();
    assert!(error.contains("depth") && error.contains("8"), "{error}");
    assert!(CopySources::prepare(&mut context, &["/".parse().unwrap()]).is_err());
    let mut context = child.capture(&commit, &"nested/".repeat(8)).unwrap();
    assert!(
        context
            .read(&ContextPath::try_from(Path::new("file")).unwrap())
            .is_ok()
    );
    assert!(child.capture(&commit, &"nested/".repeat(9)).is_err());
}
