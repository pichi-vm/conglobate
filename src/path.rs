// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Path syntax is distinct from authority to resolve or open a filesystem object.
use std::path::{Component, Path, PathBuf};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PathError {
    #[error("context path must be relative")]
    Absolute,
    #[error("context path escapes the selected context")]
    Escape,
    #[error("context file path must name a file, not the context root")]
    Empty,
    #[error("context path contains NUL")]
    Nul,
}

/// A normalized relative file locator. This does not prove symlink confinement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ContextPath(PathBuf);

impl TryFrom<&Path> for ContextPath {
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
        if normalized.as_os_str().is_empty() {
            return Err(PathError::Empty);
        }
        Ok(Self(normalized))
    }
}

impl AsRef<Path> for ContextPath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}
