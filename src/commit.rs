// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub(crate) enum CommitError {
    #[error("commit must be a full 40- or 64-digit hexadecimal object identity")]
    Commit,
    #[error("Git ref resolution: {0}")]
    Ref(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommitIdentity(Vec<u8>);
impl TryFrom<&str> for CommitIdentity {
    type Error = CommitError;
    fn try_from(text: &str) -> Result<Self, Self::Error> {
        if !matches!(text.len(), 40 | 64) {
            return Err(CommitError::Commit);
        }
        Ok(Self(hex::decode(text).map_err(|_| CommitError::Commit)?))
    }
}
impl CommitIdentity {
    pub(crate) fn resolve(repository: &Path, git_ref: &str) -> Result<Self, CommitError> {
        let repo = gix::open::Options::isolated()
            .open(repository)
            .map_err(|error| CommitError::Ref(error.to_string()))?
            .to_thread_local();
        // Full object id may be supplied directly.
        if let Ok(commit) = Self::try_from(git_ref) {
            let id = gix::ObjectId::from_hex(hex::encode(commit.bytes()).as_bytes())
                .map_err(|error| CommitError::Ref(error.to_string()))?;
            if id.kind() != repo.object_hash() {
                return Err(CommitError::Ref(
                    "commit object format differs from repository".into(),
                ));
            }
            let _ = repo
                .find_object(id)
                .map_err(|error| CommitError::Ref(error.to_string()))?
                .try_into_commit()
                .map_err(|error| CommitError::Ref(error.to_string()))?;
            return Ok(commit);
        }
        let mut reference = repo
            .find_reference(git_ref)
            .map_err(|error| CommitError::Ref(error.to_string()))?;
        let commit = reference
            .peel_to_commit()
            .map_err(|error| CommitError::Ref(error.to_string()))?;
        Self::try_from(commit.id().to_string().as_str())
            .map_err(|error| CommitError::Ref(error.to_string()))
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.0
    }
}
