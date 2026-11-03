// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Host→guest build responses: `[true, Input]` or `[false, message]`.

use crate::contract::launch::{Input, Response};
use crate::{
    path::{ContextDirectory, ContextPath, PathError},
    request::{BuildRequest, CommitIdentity, RequestError},
};
#[cfg(test)]
use ciborium::Value;
use std::io::BufRead;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ApplianceRequestError {
    #[error("appliance CBOR: {0}")]
    Decode(String),
    #[error("appliance CBOR has trailing bytes after the response")]
    TrailingBytes,
    #[error("host rejected build-input: {0}")]
    HostRejected(String),
    #[error(transparent)]
    Path(#[from] PathError),
    #[error(transparent)]
    Request(#[from] RequestError),
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

    /// Default prefetch for build-port response buffering. Responses are far
    /// smaller; tests pin smaller capacities to cover the exact-fill boundary.
    const RESPONSE_PREFETCH: usize = 8 * 1024;

    /// Decode one self-delimiting host response CBOR array from a reader.
    ///
    /// Prefetches through a [`BufReader`]. Leftover bytes already in that
    /// buffer after one response fail as trailing data. When `refill` is true, one
    /// additional `fill_buf` runs so a response that ended exactly on a prefetch
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
        let response: Response<Input> = ciborium::from_reader(&mut buffered)
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
        Self::from_response(response)
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

    /// Decode a complete response buffer (one response, no trailing bytes).
    #[cfg(test)]
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, ApplianceRequestError> {
        let mut cursor = std::io::Cursor::new(bytes);
        Self::decode_response(&mut cursor)
    }

    fn from_response(response: Response<Input>) -> Result<Self, ApplianceRequestError> {
        let input = match response {
            Response::Ok(input) => input,
            Response::Err(error) => return Err(ApplianceRequestError::HostRejected(error)),
        };
        let Input {
            r#ref: git_ref,
            plan: plan_text,
            lock: lock_text,
            context: context_text,
            location,
        } = input;
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
    ) -> Result<BuildRequest, ApplianceRequestError> {
        let commit = CommitIdentity::resolve(repository, &self.git_ref)?;
        Ok(BuildRequest::from_parts(
            Some(id_mappings),
            repository.to_path_buf(),
            commit,
            self.context,
            self.plan,
            self.lock,
            cache.to_path_buf(),
            output.to_path_buf(),
        )
        .with_location(self.location)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn encode(fields: &[(&str, Value)]) -> Vec<u8> {
        let payload = Value::Map(
            fields
                .iter()
                .map(|(key, value)| (Value::Text((*key).into()), value.clone()))
                .collect(),
        );
        let mut bytes = Vec::new();
        ciborium::into_writer(&Value::Array(vec![Value::Bool(true), payload]), &mut bytes).unwrap();
        bytes
    }

    fn required() -> Vec<(&'static str, Value)> {
        vec![
            ("ref", Value::Text("refs/heads/main".into())),
            ("plan", Value::Text("plan.yml".into())),
            ("lock", Value::Text("lock.yml".into())),
            ("context", Value::Text(".".into())),
        ]
    }

    fn host_error(message: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        ciborium::into_writer(&Response::<Input>::Err(message.into()), &mut bytes).unwrap();
        bytes
    }

    #[test]
    fn shared_host_response_is_consumed_without_translation() {
        let response = Response::Ok(Input {
            r#ref: "refs/heads/main".into(),
            plan: "pichi.yaml".into(),
            lock: "pichi.lock".into(),
            context: "project".into(),
            location: Some("https://example.invalid/repo".into()),
        });
        let mut bytes = Vec::new();
        ciborium::into_writer(&response, &mut bytes).unwrap();
        let input = ApplianceRequest::decode(&bytes).unwrap();
        assert_eq!(input.git_ref(), "refs/heads/main");
        assert_eq!(input.context().as_ref(), Path::new("project"));
        assert_eq!(input.plan().as_ref(), Path::new("pichi.yaml"));
        assert_eq!(input.lock().as_ref(), Path::new("pichi.lock"));
        assert_eq!(input.location(), Some("https://example.invalid/repo"));
    }

    #[test]
    fn accepts_required_fields_and_explicit_root_context() {
        let request = ApplianceRequest::decode(&encode(&required())).unwrap();
        assert_eq!(request.git_ref(), "refs/heads/main");
        assert_eq!(request.plan().as_ref(), Path::new("plan.yml"));
        assert_eq!(request.lock().as_ref(), Path::new("lock.yml"));
        assert_eq!(request.context().as_ref(), Path::new("."));
        assert_eq!(request.location(), None);
    }

    #[test]
    fn accepts_selected_context_and_optional_location() {
        let mut fields = required();
        fields[3] = ("context", Value::Text("project".into()));
        fields.push((
            "location",
            Value::Text("https://example.invalid/repo".into()),
        ));
        let request = ApplianceRequest::decode(&encode(&fields)).unwrap();
        assert_eq!(request.context().as_ref(), Path::new("project"));
        assert_eq!(request.location(), Some("https://example.invalid/repo"));
    }

    #[test]
    fn accepts_null_location() {
        let mut fields = required();
        fields.push(("location", Value::Null));
        assert_eq!(
            ApplianceRequest::decode(&encode(&fields))
                .unwrap()
                .location(),
            None
        );
    }

    #[test]
    fn rejects_forbidden_and_unknown_keys() {
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
            let error = ApplianceRequest::decode(&encode(&fields)).unwrap_err();
            assert!(
                matches!(&error, ApplianceRequestError::Decode(_)),
                "{key}: {error}"
            );
        }
        let mut fields = required();
        fields.push(("surprise", Value::Text("x".into())));
        let error = ApplianceRequest::decode(&encode(&fields)).unwrap_err();
        assert!(
            matches!(&error, ApplianceRequestError::Decode(_)),
            "{error}"
        );
    }

    #[test]
    fn rejects_escaping_paths_and_trailing_bytes() {
        let mut fields = required();
        // required() order: ref, plan, lock, context.
        fields[1] = ("plan", Value::Text("../escape".into()));
        assert!(matches!(
            ApplianceRequest::decode(&encode(&fields)),
            Err(ApplianceRequestError::Path(crate::path::PathError::Escape))
        ));
        let mut fields = required();
        fields[1] = ("plan", Value::Text("/etc/passwd".into()));
        assert!(matches!(
            ApplianceRequest::decode(&encode(&fields)),
            Err(ApplianceRequestError::Path(
                crate::path::PathError::Absolute
            ))
        ));
        let mut fields = required();
        fields[3] = ("context", Value::Text("../..".into()));
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
        // Production path: BufReader leftover after one response is trailing data.
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
        // Array ends exactly at the first fill; trailing is only visible after refill.
        assert!(matches!(
            ApplianceRequest::decode_response_with_capacity(&mut cursor, capacity, true),
            Err(ApplianceRequestError::TrailingBytes)
        ));
        // Exact array with refill still succeeds (Cursor EOF on second fill).
        let exact = encode(&required());
        let capacity = exact.len();
        let mut cursor = std::io::Cursor::new(exact);
        ApplianceRequest::decode_response_with_capacity(&mut cursor, capacity, true).unwrap();
    }

    #[test]
    fn rejects_ok_false_with_host_error() {
        for message in ["no plan", "", "host\0message"] {
            assert!(matches!(
                ApplianceRequest::decode(&host_error(message)),
                Err(ApplianceRequestError::HostRejected(error)) if error == message
            ));
        }
    }

    #[test]
    fn rejects_failure_with_success_payload() {
        let mut bytes = encode(&required());
        assert_eq!(bytes[1], 0xf5);
        bytes[1] = 0xf4;
        assert!(matches!(
            ApplianceRequest::decode(&bytes),
            Err(ApplianceRequestError::Decode(_))
        ));
    }

    #[test]
    fn decode_response_reads_one_self_delimiting_array() {
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
