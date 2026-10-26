// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Flat-map CBOR appliance request: accept-list + reject-list with key names.

use crate::{
    path::{ContextDirectory, ContextPath, PathError},
    request::{BuildRequest, CommitIdentity, RequestError},
};
use ciborium::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum ApplianceRequestError {
    #[error("appliance CBOR: {0}")]
    Decode(String),
    #[error("appliance CBOR rejects field {0}")]
    RejectedField(&'static str),
    #[error("appliance CBOR rejects unknown field {0}")]
    UnknownField(String),
    #[error("appliance CBOR missing required field {0}")]
    MissingField(&'static str),
    #[error("appliance CBOR field {0} has invalid type")]
    InvalidType(&'static str),
    #[error("appliance CBOR field {0} is empty or contains NUL")]
    InvalidString(&'static str),
    #[error("appliance CBOR has trailing bytes after the request map")]
    TrailingBytes,
    #[error(transparent)]
    Path(#[from] PathError),
    #[error(transparent)]
    Request(#[from] RequestError),
    #[error("Git ref resolution: {0}")]
    Ref(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApplianceRequest {
    git_ref: String,
    recipe: ContextPath,
    lock: ContextPath,
    subdir: ContextDirectory,
    location: Option<String>,
}

impl ApplianceRequest {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn git_ref(&self) -> &str {
        &self.git_ref
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn recipe(&self) -> &ContextPath {
        &self.recipe
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn lock(&self) -> &ContextPath {
        &self.lock
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn subdir(&self) -> &ContextDirectory {
        &self.subdir
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn location(&self) -> Option<&str> {
        self.location.as_deref()
    }

    /// Decode a one-shot CBOR map. Unknown and forbidden keys name the offender.
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, ApplianceRequestError> {
        let mut cursor = std::io::Cursor::new(bytes);
        let value: Value = ciborium::from_reader(&mut cursor)
            .map_err(|error| ApplianceRequestError::Decode(error.to_string()))?;
        if cursor.position() != bytes.len() as u64 {
            return Err(ApplianceRequestError::TrailingBytes);
        }
        let Value::Map(entries) = value else {
            return Err(ApplianceRequestError::Decode(
                "top-level value must be a map".into(),
            ));
        };
        let mut fields = BTreeMap::new();
        for (key, value) in entries {
            let name = match key {
                Value::Text(text) => text,
                _ => {
                    return Err(ApplianceRequestError::Decode(
                        "map keys must be text".into(),
                    ));
                }
            };
            if fields.insert(name.clone(), value).is_some() {
                return Err(ApplianceRequestError::Decode(format!(
                    "duplicate field {name}"
                )));
            }
        }
        for forbidden in [
            "commit",
            "output",
            "repository",
            "cache",
            "uid_map",
            "gid_map",
            "network_namespace",
            "resolv_conf",
            "formatter_runtime",
            "formatter-runtime",
            "repo",
            "mount",
            "tag",
        ] {
            if fields.contains_key(forbidden) {
                return Err(ApplianceRequestError::RejectedField(forbidden));
            }
        }
        for key in fields.keys() {
            if !matches!(
                key.as_str(),
                "ref" | "recipe" | "lock" | "subdir" | "location"
            ) {
                return Err(ApplianceRequestError::UnknownField(key.clone()));
            }
        }
        let git_ref = take_string(&mut fields, "ref", true)?.unwrap();
        let recipe_text = take_string(&mut fields, "recipe", true)?.unwrap();
        let lock_text = take_string(&mut fields, "lock", true)?.unwrap();
        let subdir_text =
            take_string(&mut fields, "subdir", false)?.unwrap_or_else(|| ".".to_owned());
        let location = take_string(&mut fields, "location", false)?;
        let recipe = ContextPath::try_from(Path::new(&recipe_text))?;
        let lock = ContextPath::try_from(Path::new(&lock_text))?;
        let subdir = ContextDirectory::try_from(Path::new(&subdir_text))?;
        Ok(Self {
            git_ref,
            recipe,
            lock,
            subdir,
            location,
        })
    }

    /// Resolve `ref` against a repository and build the shared engine request.
    pub(crate) fn into_build_request(
        self,
        repository: &Path,
        cache: &Path,
        output: &Path,
        id_mappings: crate::isolation::IdMappings,
        network_namespace: PathBuf,
        resolv_conf: PathBuf,
    ) -> Result<BuildRequest, ApplianceRequestError> {
        let commit = resolve_ref(repository, &self.git_ref)?;
        Ok(BuildRequest::from_parts(
            Some(id_mappings),
            repository.to_path_buf(),
            commit,
            self.subdir,
            self.recipe,
            self.lock,
            cache.to_path_buf(),
            output.to_path_buf(),
            Some(resolv_conf),
            Some(network_namespace),
        ))
    }
}

fn take_string(
    fields: &mut BTreeMap<String, Value>,
    name: &'static str,
    required: bool,
) -> Result<Option<String>, ApplianceRequestError> {
    match fields.remove(name) {
        None if required => Err(ApplianceRequestError::MissingField(name)),
        None => Ok(None),
        Some(Value::Text(text)) => {
            if text.is_empty() || text.contains('\0') {
                return Err(ApplianceRequestError::InvalidString(name));
            }
            Ok(Some(text))
        }
        Some(_) => Err(ApplianceRequestError::InvalidType(name)),
    }
}

fn resolve_ref(repository: &Path, git_ref: &str) -> Result<CommitIdentity, ApplianceRequestError> {
    let repo = gix::open::Options::isolated()
        .open(repository)
        .map_err(|error| ApplianceRequestError::Ref(error.to_string()))?
        .to_thread_local();
    // Full object id may be supplied directly.
    if let Ok(commit) = CommitIdentity::try_from(git_ref) {
        let id = gix::ObjectId::from_hex(hex::encode(commit.bytes()).as_bytes())
            .map_err(|error| ApplianceRequestError::Ref(error.to_string()))?;
        if id.kind() != repo.object_hash() {
            return Err(ApplianceRequestError::Ref(
                "commit object format differs from repository".into(),
            ));
        }
        let _ = repo
            .find_object(id)
            .map_err(|error| ApplianceRequestError::Ref(error.to_string()))?
            .try_into_commit()
            .map_err(|error| ApplianceRequestError::Ref(error.to_string()))?;
        return Ok(commit);
    }
    let mut reference = repo
        .find_reference(git_ref)
        .map_err(|error| ApplianceRequestError::Ref(error.to_string()))?;
    let commit = reference
        .peel_to_commit()
        .map_err(|error| ApplianceRequestError::Ref(error.to_string()))?;
    CommitIdentity::try_from(commit.id().to_string().as_str())
        .map_err(|error| ApplianceRequestError::Ref(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn encode(map: &[(&str, Value)]) -> Vec<u8> {
        let value = Value::Map(
            map.iter()
                .map(|(k, v)| (Value::Text((*k).into()), v.clone()))
                .collect(),
        );
        let mut bytes = Vec::new();
        ciborium::into_writer(&value, &mut bytes).unwrap();
        bytes
    }

    fn required() -> Vec<(&'static str, Value)> {
        vec![
            ("ref", Value::Text("refs/heads/main".into())),
            ("recipe", Value::Text("recipe.yml".into())),
            ("lock", Value::Text("lock.yml".into())),
        ]
    }

    #[test]
    fn accepts_required_fields_and_defaults_subdir() {
        let request = ApplianceRequest::decode(&encode(&required())).unwrap();
        assert_eq!(request.git_ref(), "refs/heads/main");
        assert_eq!(request.recipe().as_ref(), Path::new("recipe.yml"));
        assert_eq!(request.lock().as_ref(), Path::new("lock.yml"));
        assert_eq!(request.subdir().as_ref(), Path::new(""));
        assert_eq!(request.location(), None);
    }

    #[test]
    fn accepts_optional_subdir_and_location() {
        let mut fields = required();
        fields.push(("subdir", Value::Text("project".into())));
        fields.push((
            "location",
            Value::Text("https://example.invalid/repo".into()),
        ));
        let request = ApplianceRequest::decode(&encode(&fields)).unwrap();
        assert_eq!(request.subdir().as_ref(), Path::new("project"));
        assert_eq!(request.location(), Some("https://example.invalid/repo"));
    }

    #[test]
    fn rejects_forbidden_and_unknown_keys_by_name() {
        for key in [
            "commit",
            "output",
            "uid_map",
            "gid_map",
            "network_namespace",
            "resolv_conf",
            "formatter_runtime",
            "repository",
            "cache",
        ] {
            let mut fields = required();
            fields.push((key, Value::Text("x".into())));
            let error = ApplianceRequest::decode(&encode(&fields))
                .unwrap_err()
                .to_string();
            assert!(error.contains(key), "{key}: {error}");
        }
        let mut fields = required();
        fields.push(("surprise", Value::Text("x".into())));
        let error = ApplianceRequest::decode(&encode(&fields))
            .unwrap_err()
            .to_string();
        assert!(error.contains("surprise"), "{error}");
    }

    #[test]
    fn rejects_escaping_paths_and_trailing_bytes() {
        let mut fields = required();
        fields[1] = ("recipe", Value::Text("../escape".into()));
        assert!(ApplianceRequest::decode(&encode(&fields)).is_err());
        let mut bytes = encode(&required());
        bytes.push(0);
        assert!(matches!(
            ApplianceRequest::decode(&bytes),
            Err(ApplianceRequestError::TrailingBytes)
        ));
    }

    #[test]
    fn resolves_ref_to_commit_on_temp_repository() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path();
        assert!(
            Command::new("git")
                .args(["init", "-b", "main"])
                .current_dir(repo)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["config", "user.email", "t@example.invalid"])
                .current_dir(repo)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["config", "user.name", "t"])
                .current_dir(repo)
                .status()
                .unwrap()
                .success()
        );
        std::fs::write(repo.join("recipe.yml"), "x").unwrap();
        std::fs::write(repo.join("lock.yml"), "y").unwrap();
        assert!(
            Command::new("git")
                .args(["add", "."])
                .current_dir(repo)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["commit", "-m", "c"])
                .current_dir(repo)
                .status()
                .unwrap()
                .success()
        );
        let head = String::from_utf8(
            Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(repo)
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_owned();
        let request = ApplianceRequest::decode(&encode(&required())).unwrap();
        let build = request
            .into_build_request(
                repo,
                Path::new("/run/pichi/cache"),
                Path::new("/run/pichi/output"),
                super::super::mounts::id_mappings(),
                PathBuf::from("/run/pichi/netns"),
                PathBuf::from("/run/pichi/resolv.conf"),
            )
            .unwrap();
        assert_eq!(
            build.commit().bytes(),
            CommitIdentity::try_from(head.as_str()).unwrap().bytes()
        );
        assert_eq!(build.repository(), repo);
        assert_eq!(build.cache(), Path::new("/run/pichi/cache"));
        assert!(build.id_mappings().is_some());
    }
}
