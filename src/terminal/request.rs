// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Host→guest build responses: `[true, Input]` or `[false, message]`.

use crate::contract::launch::{Input, Response};
#[cfg(test)]
use ciborium::Value;
use std::io::BufRead;

#[derive(Debug, thiserror::Error)]
pub(crate) enum TerminalRequestError {
    #[error("terminal CBOR: {0}")]
    Decode(String),
    #[error("terminal CBOR has trailing bytes after the response")]
    TrailingBytes,
    #[error("host rejected build-input: {0}")]
    HostRejected(String),
}

/// Default prefetch for build-port response buffering. Responses are far
/// smaller; tests pin smaller capacities to cover the exact-fill boundary.
const RESPONSE_PREFETCH: usize = 8 * 1024;

/// Production build-port path: buffered decode without a blocking refill,
/// then a zero-timeout poll for bytes still waiting on the port.
pub(crate) fn decode_response_from_port(
    file: &mut std::fs::File,
) -> Result<Input, TerminalRequestError> {
    let request = decode_response_with_capacity(file, RESPONSE_PREFETCH, false)?;
    reject_immediate_port_trailing(file)?;
    Ok(request)
}

fn decode_response_with_capacity(
    reader: &mut impl std::io::Read,
    capacity: usize,
    refill: bool,
) -> Result<Input, TerminalRequestError> {
    let mut buffered = std::io::BufReader::with_capacity(capacity, reader);
    let response: Response<Input> = ciborium::from_reader(&mut buffered)
        .map_err(|error| TerminalRequestError::Decode(error.to_string()))?;
    if !buffered.buffer().is_empty() {
        return Err(TerminalRequestError::TrailingBytes);
    }
    if refill {
        let more = buffered
            .fill_buf()
            .map_err(|error| TerminalRequestError::Decode(error.to_string()))?;
        if !more.is_empty() {
            return Err(TerminalRequestError::TrailingBytes);
        }
    }
    match response {
        Response::Ok(input) => Ok(input),
        Response::Err(error) => Err(TerminalRequestError::HostRejected(error)),
    }
}

fn reject_immediate_port_trailing(file: &mut std::fs::File) -> Result<(), TerminalRequestError> {
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
    .map_err(|error| TerminalRequestError::Decode(error.to_string()))?;
    let events = fds[0].revents();
    if events.intersects(PollFlags::ERR | PollFlags::NVAL) {
        return Err(TerminalRequestError::Decode(
            "build-port poll returned ERR/NVAL".into(),
        ));
    }
    if !events.intersects(PollFlags::IN | PollFlags::HUP) {
        return Ok(());
    }
    let mut byte = [0u8; 1];
    match file.read(&mut byte) {
        Ok(0) => Ok(()),
        Ok(_) => Err(TerminalRequestError::TrailingBytes),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
        Err(error) => Err(TerminalRequestError::Decode(error.to_string())),
    }
}

/// Decode a complete response buffer (one response, no trailing bytes).
#[cfg(test)]
pub(crate) fn decode(bytes: &[u8]) -> Result<Input, TerminalRequestError> {
    let mut cursor = std::io::Cursor::new(bytes);
    decode_response_with_capacity(&mut cursor, RESPONSE_PREFETCH, true)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let input = decode(&bytes).unwrap();
        assert_eq!(input.r#ref, "refs/heads/main");
        assert_eq!(input.context, "project");
        assert_eq!(input.plan, "pichi.yaml");
        assert_eq!(input.lock, "pichi.lock");
        assert_eq!(
            input.location.as_deref(),
            Some("https://example.invalid/repo")
        );
    }

    #[test]
    fn accepts_required_fields_and_explicit_root_context() {
        let request = decode(&encode(&required())).unwrap();
        assert_eq!(request.r#ref, "refs/heads/main");
        assert_eq!(request.plan, "plan.yml");
        assert_eq!(request.lock, "lock.yml");
        assert_eq!(request.context, ".");
        assert_eq!(request.location.as_deref(), None);
    }

    #[test]
    fn accepts_selected_context_and_optional_location() {
        let mut fields = required();
        fields[3] = ("context", Value::Text("project".into()));
        fields.push((
            "location",
            Value::Text("https://example.invalid/repo".into()),
        ));
        let request = decode(&encode(&fields)).unwrap();
        assert_eq!(request.context, "project");
        assert_eq!(
            request.location.as_deref(),
            Some("https://example.invalid/repo")
        );
    }

    #[test]
    fn accepts_null_location() {
        let mut fields = required();
        fields.push(("location", Value::Null));
        assert_eq!(decode(&encode(&fields)).unwrap().location.as_deref(), None);
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
            let error = decode(&encode(&fields)).unwrap_err();
            assert!(
                matches!(&error, TerminalRequestError::Decode(_)),
                "{key}: {error}"
            );
        }
        let mut fields = required();
        fields.push(("surprise", Value::Text("x".into())));
        let error = decode(&encode(&fields)).unwrap_err();
        assert!(matches!(&error, TerminalRequestError::Decode(_)), "{error}");
    }

    #[test]
    fn rejects_trailing_bytes() {
        let mut bytes = encode(&required());
        bytes.push(0);
        assert!(matches!(
            decode(&bytes),
            Err(TerminalRequestError::TrailingBytes)
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
            decode_response_with_capacity(&mut cursor, capacity, true),
            Err(TerminalRequestError::TrailingBytes)
        ));
        // Exact array with refill still succeeds (Cursor EOF on second fill).
        let exact = encode(&required());
        let capacity = exact.len();
        let mut cursor = std::io::Cursor::new(exact);
        decode_response_with_capacity(&mut cursor, capacity, true).unwrap();
    }

    #[test]
    fn rejects_ok_false_with_host_error() {
        for message in ["no plan", "", "host\0message"] {
            assert!(matches!(
                decode(&host_error(message)),
                Err(TerminalRequestError::HostRejected(error)) if error == message
            ));
        }
    }

    #[test]
    fn rejects_failure_with_success_payload() {
        let mut bytes = encode(&required());
        assert_eq!(bytes[1], 0xf5);
        bytes[1] = 0xf4;
        assert!(matches!(
            decode(&bytes),
            Err(TerminalRequestError::Decode(_))
        ));
    }

    #[test]
    fn decode_response_reads_one_self_delimiting_array() {
        let exact = encode(&required());
        let mut cursor = std::io::Cursor::new(exact.as_slice());
        let request = decode_response_with_capacity(&mut cursor, RESPONSE_PREFETCH, true).unwrap();
        assert_eq!(request.r#ref, "refs/heads/main");
        assert_eq!(cursor.position(), exact.len() as u64);
    }
}
