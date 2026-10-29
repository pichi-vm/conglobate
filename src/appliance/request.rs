// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Host→guest build-input response CBOR: envelope + flat build fields.
//!
//! The response is the library's [`BuildReply`]: a flat map with `v`/`ok`
//! plus build-input fields on success (`ref`/`plan`/`lock`/optional
//! `context`/`location`), or `error` on failure. Duplicate keys and known
//! host-only field names are rejected by name before the typed decode.

use crate::{
    path::{ContextDirectory, ContextPath, PathError},
    request::{BuildRequest, CommitIdentity, RequestError},
};
use ciborium::Value;
use contract::console::BuildReply;
use std::io::BufRead;
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
    #[error("appliance CBOR field {0} is empty or contains NUL")]
    InvalidString(&'static str),
    #[error("build location must be a credential-free Git locator")]
    InvalidLocation,
    #[error("appliance CBOR has trailing bytes after the response map")]
    TrailingBytes,
    #[error("host rejected build-input: {0}")]
    HostRejected(String),
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
    plan: ContextPath,
    lock: ContextPath,
    context: ContextDirectory,
    location: Option<String>,
}

impl ApplianceRequest {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn git_ref(&self) -> &str {
        &self.git_ref
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn plan(&self) -> &ContextPath {
        &self.plan
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn lock(&self) -> &ContextPath {
        &self.lock
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn context(&self) -> &ContextDirectory {
        &self.context
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn location(&self) -> Option<&str> {
        self.location.as_deref()
    }

    /// Default prefetch for build-port response buffering. v0 maps are far
    /// smaller; tests pin smaller capacities to cover the exact-fill boundary.
    const RESPONSE_PREFETCH: usize = 8 * 1024;

    /// Decode one self-delimiting host response CBOR map from a reader.
    ///
    /// Prefetches through a [`BufReader`]. Leftover bytes already in that
    /// buffer after one map fail as trailing data. When `refill` is true, one
    /// additional `fill_buf` runs so a map that ended exactly on a prefetch
    /// boundary still rejects trailing bytes already available from the
    /// underlying reader (Cursor / chunked test readers return immediately;
    /// do not use `refill` on a blocking build-port file — use
    /// [`Self::decode_response_from_port`] instead).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn decode_response(
        reader: &mut impl std::io::Read,
    ) -> Result<Self, ApplianceRequestError> {
        Self::decode_response_with_capacity(reader, Self::RESPONSE_PREFETCH, true)
    }

    /// Production build-port path: buffered decode without a blocking refill,
    /// then a zero-timeout poll for bytes still waiting on the port.
    pub(crate) fn decode_response_from_port(
        file: &mut std::fs::File,
    ) -> Result<Self, ApplianceRequestError> {
        let request = Self::decode_response_with_capacity(file, Self::RESPONSE_PREFETCH, false)?;
        Self::reject_immediate_port_trailing(file)?;
        Ok(request)
    }

    fn decode_response_with_capacity(
        reader: &mut impl std::io::Read,
        capacity: usize,
        refill: bool,
    ) -> Result<Self, ApplianceRequestError> {
        let mut buffered = std::io::BufReader::with_capacity(capacity, reader);
        let value: Value = ciborium::from_reader(&mut buffered)
            .map_err(|error| ApplianceRequestError::Decode(error.to_string()))?;
        if !buffered.buffer().is_empty() {
            return Err(ApplianceRequestError::TrailingBytes);
        }
        if refill {
            let more = buffered
                .fill_buf()
                .map_err(|error| ApplianceRequestError::Decode(error.to_string()))?;
            if !more.is_empty() {
                return Err(ApplianceRequestError::TrailingBytes);
            }
        }
        Self::from_response_value(value)
    }

    fn reject_immediate_port_trailing(
        file: &mut std::fs::File,
    ) -> Result<(), ApplianceRequestError> {
        use rustix::event::{PollFd, PollFlags, Timespec, poll};
        use std::io::Read;
        let mut fds = [PollFd::new(&*file, PollFlags::IN)];
        poll(
            &mut fds,
            Some(&Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            }),
        )
        .map_err(|error| ApplianceRequestError::Decode(error.to_string()))?;
        let events = fds[0].revents();
        if events.intersects(PollFlags::ERR | PollFlags::NVAL) {
            return Err(ApplianceRequestError::Decode(
                "build-port poll returned ERR/NVAL".into(),
            ));
        }
        if !events.intersects(PollFlags::IN | PollFlags::HUP) {
            return Ok(());
        }
        let mut byte = [0u8; 1];
        match file.read(&mut byte) {
            Ok(0) => Ok(()),
            Ok(_) => Err(ApplianceRequestError::TrailingBytes),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
            Err(error) => Err(ApplianceRequestError::Decode(error.to_string())),
        }
    }

    /// Decode a complete response buffer (one map, no trailing bytes).
    #[cfg(test)]
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, ApplianceRequestError> {
        let mut cursor = std::io::Cursor::new(bytes);
        Self::decode_response(&mut cursor)
    }

    fn from_response_value(value: Value) -> Result<Self, ApplianceRequestError> {
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
            "op",
        ] {
            if fields.contains_key(forbidden) {
                return Err(ApplianceRequestError::RejectedField(forbidden));
            }
        }
        // Field names are now known to be unique, so the typed reply sees
        // exactly the map that was sent.
        let value = Value::Map(
            fields
                .into_iter()
                .map(|(name, value)| (Value::Text(name), value))
                .collect(),
        );
        let input = match value
            .deserialized::<BuildReply>()
            .map_err(|error| ApplianceRequestError::Decode(error.to_string()))?
        {
            BuildReply::Success(input) => input,
            BuildReply::Failure { error } => {
                return Err(ApplianceRequestError::HostRejected(error));
            }
        };
        let git_ref = checked_string(input.git_ref, "ref")?;
        let plan_text = checked_string(input.plan, "plan")?;
        let lock_text = checked_string(input.lock, "lock")?;
        let context_text = input
            .context
            .map(|context| checked_string(context, "context"))
            .transpose()?
            .unwrap_or_else(|| ".".to_owned());
        let location = input
            .location
            .map(|location| checked_string(location, "location"))
            .transpose()?;
        if let Some(location) = &location {
            // Parser errors can include the input; never surface them here.
            let locator = gix::url::parse(location.as_bytes().into())
                .map_err(|_| ApplianceRequestError::InvalidLocation)?;
            if locator.password().is_some()
                || (locator.user().is_some() && locator.scheme != gix::url::Scheme::Ssh)
            {
                return Err(ApplianceRequestError::InvalidLocation);
            }
        }
        let plan = ContextPath::try_from(Path::new(&plan_text))?;
        let lock = ContextPath::try_from(Path::new(&lock_text))?;
        let context = ContextDirectory::try_from(Path::new(&context_text))?;
        Ok(Self {
            git_ref,
            plan,
            lock,
            context,
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
            self.context,
            self.plan,
            self.lock,
            cache.to_path_buf(),
            output.to_path_buf(),
            Some(resolv_conf),
            Some(network_namespace),
        )
        .with_location(self.location))
    }
}

fn checked_string(text: String, name: &'static str) -> Result<String, ApplianceRequestError> {
    if text.is_empty() || text.contains('\0') {
        return Err(ApplianceRequestError::InvalidString(name));
    }
    Ok(text)
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
            ("v", Value::Integer(1.into())),
            ("ok", Value::Bool(true)),
            ("ref", Value::Text("refs/heads/main".into())),
            ("plan", Value::Text("plan.yml".into())),
            ("lock", Value::Text("lock.yml".into())),
        ]
    }

    fn host_error(message: &str) -> Vec<u8> {
        encode(&[
            ("v", Value::Integer(1.into())),
            ("ok", Value::Bool(false)),
            ("error", Value::Text(message.into())),
        ])
    }

    #[test]
    fn accepts_required_fields_and_defaults_context() {
        let request = ApplianceRequest::decode(&encode(&required())).unwrap();
        assert_eq!(request.git_ref(), "refs/heads/main");
        assert_eq!(request.plan().as_ref(), Path::new("plan.yml"));
        assert_eq!(request.lock().as_ref(), Path::new("lock.yml"));
        assert_eq!(request.context().as_ref(), Path::new(""));
        assert_eq!(request.location(), None);
    }

    #[test]
    fn accepts_optional_context_and_location() {
        let mut fields = required();
        fields.push(("context", Value::Text("project".into())));
        fields.push((
            "location",
            Value::Text("https://example.invalid/repo".into()),
        ));
        let request = ApplianceRequest::decode(&encode(&fields)).unwrap();
        assert_eq!(request.context().as_ref(), Path::new("project"));
        assert_eq!(request.location(), Some("https://example.invalid/repo"));
    }

    #[test]
    fn rejects_credentials_without_echoing_the_locator() {
        for location in [
            "https://secret@example.invalid/repo",
            "https://user:secret@example.invalid/repo",
            "ssh://user:secret@example.invalid/repo",
        ] {
            let mut fields = required();
            fields.push(("location", Value::Text(location.into())));
            let error = ApplianceRequest::decode(&encode(&fields))
                .unwrap_err()
                .to_string();
            assert!(error.contains("credential-free"));
            assert!(!error.contains("secret"));
        }
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
        // required() order: v, ok, ref, plan, lock — index 3 is plan.
        fields[3] = ("plan", Value::Text("../escape".into()));
        assert!(matches!(
            ApplianceRequest::decode(&encode(&fields)),
            Err(ApplianceRequestError::Path(crate::path::PathError::Escape))
        ));
        let mut fields = required();
        fields[3] = ("plan", Value::Text("/etc/passwd".into()));
        assert!(matches!(
            ApplianceRequest::decode(&encode(&fields)),
            Err(ApplianceRequestError::Path(
                crate::path::PathError::Absolute
            ))
        ));
        let mut fields = required();
        fields.push(("context", Value::Text("../..".into())));
        assert!(matches!(
            ApplianceRequest::decode(&encode(&fields)),
            Err(ApplianceRequestError::Path(crate::path::PathError::Escape))
        ));
        let mut bytes = encode(&required());
        bytes.push(0);
        assert!(matches!(
            ApplianceRequest::decode(&bytes),
            Err(ApplianceRequestError::TrailingBytes)
        ));
        // Production path: BufReader leftover after one map is trailing data.
        let mut cursor = std::io::Cursor::new(bytes);
        assert!(matches!(
            ApplianceRequest::decode_response(&mut cursor),
            Err(ApplianceRequestError::TrailingBytes)
        ));
    }

    #[test]
    fn rejects_trailing_bytes_at_prefetch_boundary() {
        let map = encode(&required());
        let capacity = map.len();
        assert!(capacity > 0);
        let mut bytes = map;
        bytes.push(0xFF);
        let mut cursor = std::io::Cursor::new(bytes);
        // Map ends exactly at the first fill; trailing is only visible after refill.
        assert!(matches!(
            ApplianceRequest::decode_response_with_capacity(&mut cursor, capacity, true),
            Err(ApplianceRequestError::TrailingBytes)
        ));
        // Exact map with refill still succeeds (Cursor EOF on second fill).
        let exact = encode(&required());
        let capacity = exact.len();
        let mut cursor = std::io::Cursor::new(exact);
        ApplianceRequest::decode_response_with_capacity(&mut cursor, capacity, true).unwrap();
    }

    #[test]
    fn rejects_ok_false_with_host_error() {
        let error = ApplianceRequest::decode(&host_error("no plan"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("no plan"), "{error}");
    }

    #[test]
    fn rejects_ok_false_with_success_fields() {
        let error = ApplianceRequest::decode(&encode(&[
            ("v", Value::Integer(1.into())),
            ("ok", Value::Bool(false)),
            ("error", Value::Text("nope".into())),
            ("ref", Value::Text("refs/heads/main".into())),
        ]))
        .unwrap_err()
        .to_string();
        assert!(error.contains("ref"), "{error}");
    }

    #[test]
    fn decode_response_reads_one_self_delimiting_map() {
        let exact = encode(&required());
        let mut cursor = std::io::Cursor::new(exact.as_slice());
        let request = ApplianceRequest::decode_response(&mut cursor).unwrap();
        assert_eq!(request.git_ref(), "refs/heads/main");
        assert_eq!(cursor.position(), exact.len() as u64);
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
        std::fs::write(repo.join("plan.yml"), "x").unwrap();
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
        assert_eq!(build.location(), None);
    }

    #[test]
    fn location_is_forwarded_as_build_provenance() {
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
        std::fs::write(repo.join("plan.yml"), "x").unwrap();
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
        let mut fields = required();
        fields.push((
            "location",
            Value::Text("https://example.invalid/repo".into()),
        ));
        let request = ApplianceRequest::decode(&encode(&fields)).unwrap();
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
        assert_eq!(build.repository(), repo);
        assert_eq!(
            build.location(),
            Some("https://example.invalid/repo"),
            "CBOR location must reach BuildRequest for build provenance"
        );
    }
}
