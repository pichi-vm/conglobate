// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Path syntax is distinct from authority to resolve or open a filesystem object.
use std::{
    marker::PhantomData,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum PathError {
    #[error("context path must be relative")]
    Absolute,
    #[error("context path escapes the selected context")]
    Escape,
    #[error("context file path must name a file, not the context root")]
    Empty,
    #[error("context path contains NUL")]
    Nul,
}

/// A relative locator whose components must still be resolved in context.
/// Lexical checks do not prove symlink confinement; preserve `..` for resolution.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RelativePath<K: PathKind> {
    path: PathBuf,
    kind: PhantomData<K>,
}

mod private {
    pub(crate) trait Sealed {}
}
pub(crate) trait PathKind: private::Sealed {
    const ALLOW_ROOT: bool;
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FilePath;
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct DirectoryPath;
impl private::Sealed for FilePath {}
impl private::Sealed for DirectoryPath {}
impl PathKind for FilePath {
    const ALLOW_ROOT: bool = false;
}
impl PathKind for DirectoryPath {
    const ALLOW_ROOT: bool = true;
}
pub(crate) type ContextPath = RelativePath<FilePath>;
pub(crate) type ContextDirectory = RelativePath<DirectoryPath>;

impl<K: PathKind> TryFrom<&Path> for RelativePath<K> {
    type Error = PathError;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        if path.as_os_str().as_encoded_bytes().contains(&0) {
            return Err(PathError::Nul);
        }
        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                Component::RootDir | Component::Prefix(_) => return Err(PathError::Absolute),
                Component::ParentDir => {
                    if !normalized.pop() {
                        return Err(PathError::Escape);
                    }
                }
                Component::CurDir => {}
                Component::Normal(part) => normalized.push(part),
            }
        }
        if normalized.as_os_str().is_empty() && !K::ALLOW_ROOT {
            return Err(PathError::Empty);
        }
        Ok(Self {
            path: path.to_path_buf(),
            kind: PhantomData,
        })
    }
}

impl<K: PathKind> AsRef<Path> for RelativePath<K> {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}
