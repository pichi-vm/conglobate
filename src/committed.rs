// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    commit::CommitIdentity,
    context::CapturedContext,
    path::{ContextDirectory, ContextPath, git_path},
    plan::Plan,
};
use std::path::Path;

impl Plan {
    /// Read a plan from verified committed objects and return its pinned commit.
    ///
    /// Context and plan use Git slash paths, never working-tree file contents.
    /// On Windows, non-UTF-8 paths and names containing backslashes or colons
    /// are rejected rather than interpreted as native path syntax.
    pub fn from_git(
        repository: &Path,
        git_ref: &str,
        context: &str,
        plan: &str,
    ) -> Result<(String, Self), String> {
        let commit = CommitIdentity::resolve(repository, git_ref).map_err(|e| e.to_string())?;
        let directory =
            ContextDirectory::try_from(git_path(context.as_bytes()).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let path = ContextPath::try_from(git_path(plan.as_bytes()).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let mut captured =
            CapturedContext::capture(repository, &commit, &directory).map_err(|e| e.to_string())?;
        let file = captured.read(&path).map_err(|e| e.to_string())?;
        let plan =
            Self::from_yaml(file.text().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        Ok((hex::encode(commit.bytes()), plan))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::context::tests::Repository;

    const OFFLINE: &[u8] = b"version: 1\nimage: registry.test/base:1\n";
    const ONLINE: &[u8] = b"version: 1\nstages:\n  selected:\n    extend: registry.test/base:1\n    steps:\n      - exec:\n          command: 'true'\n          network: true\nimage: selected\n";

    #[test]
    fn probe_pins_selected_commit_and_ignores_dirty_working_files() {
        for format in ["sha1", "sha256"] {
            let repo = Repository::new(format);
            repo.write("context/plan.yml", OFFLINE);
            let first = repo.commit();
            repo.git(&["branch", "selected"]);
            repo.write("context/plan.yml", ONLINE);
            repo.commit();
            repo.write("context/plan.yml", b"not a plan");
            let (pin, plan) =
                Plan::from_git(repo.0.path(), "selected", "context", "plan.yml").unwrap();
            assert_eq!(pin, hex::encode(first.bytes()));
            assert!(!plan.requires_network().unwrap());
            repo.git(&["branch", "-f", "selected", "HEAD"]);
            assert!(
                !Plan::from_git(repo.0.path(), &pin, "context", "plan.yml")
                    .unwrap()
                    .1
                    .requires_network()
                    .unwrap()
            );
            assert!(
                Plan::from_git(repo.0.path(), "HEAD", "context", "plan.yml")
                    .unwrap()
                    .1
                    .requires_network()
                    .unwrap()
            );
        }
    }

    #[test]
    fn probe_uses_committed_links_and_gitlinks_not_working_metadata() {
        let child = Repository::new("sha1");
        child.write("dir/plan.yml", OFFLINE);
        child.commit();
        let repo = Repository::new("sha1");
        repo.module(&child, "vendor/lib", "different-name");
        std::os::unix::fs::symlink("vendor/lib/dir", repo.0.path().join("context")).unwrap();
        repo.commit();
        repo.write(".gitmodules", b"invalid working metadata");
        child.write("dir/plan.yml", ONLINE);
        child.commit();
        let (_, plan) = Plan::from_git(repo.0.path(), "HEAD", "context", "plan.yml").unwrap();
        assert!(!plan.requires_network().unwrap());
        assert!(Plan::from_git(repo.0.path(), "HEAD", ".", "../plan.yml").is_err());
        assert!(Plan::from_git(repo.0.path(), "HEAD", ".", "vendor").is_err());
    }
}
