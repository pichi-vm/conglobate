// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Check that final DTB edits are compatible with the firmware's measured input.

use crate::Error;
use ciborium::Value;

/// An unchanged DTB requires no resealing. Changes require a known PMI target
/// contract accepting an external measured DTB, with enough reserved capacity.
/// Signed SEV launch identities require resealing, which is not implemented.
pub fn validate_transform(pmi: &[u8], before: &[u8], after: &[u8]) -> Result<(), Error> {
    if before == after {
        return Ok(());
    }
    let pe = goblin::pe::PE::parse(pmi).map_err(invalid)?;
    let mut targets = std::collections::BTreeSet::new();
    for section in &pe.sections {
        let name = section.name().map_err(invalid)?;
        if !name.starts_with(".pmi.") {
            continue;
        }
        if !targets.insert(name) {
            return Err(invalid("duplicate PMI target section"));
        }
        let start = section.pointer_to_raw_data as usize;
        let end = start
            .checked_add(section.size_of_raw_data as usize)
            .ok_or_else(|| invalid("PMI section overflow"))?;
        let data = pmi
            .get(start..end)
            .ok_or_else(|| invalid("PMI section exceeds file"))?;
        match name {
            ".pmi.vm" => {
                decode::<pmi::vm::Spec<Value>>(data)?;
            }
            ".pmi.sev" => {
                decode::<pmi::sev::Spec>(data)?;
            }
            ".pmi.tdx" => {
                decode::<pmi::tdx::Spec>(data)?;
            }
            ".pmi.cca" => {
                decode::<pmi::cca::Spec>(data)?;
            }
            _ => return Err(invalid(format!("DTB edits unsupported for target {name}"))),
        }
        let value: Value = decode(data)?;
        unique_maps(&value)?;
        if field(&value, "sev:id").is_some_and(|value| !matches!(value, Value::Null)) {
            return Err(invalid(
                "DTB edit would invalidate a signed SEV launch identity; generate the final DTB before signing",
            ));
        }
        let actions = field(&value, "actions")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("PMI has no actions"))?;
        let fills: Vec<_> = actions
            .iter()
            .filter(|action| {
                field(action, "type").and_then(Value::as_text) == Some("fill")
                    && field(action, "kind").and_then(Value::as_text) == Some("dt:dtb")
            })
            .collect();
        if fills.len() != 1 {
            return Err(invalid(format!(
                "{name} does not accept one external measured DTB"
            )));
        }
        let reserved = field(fills[0], "section")
            .and_then(Value::as_text)
            .ok_or_else(|| invalid("DTB fill lacks section"))?;
        let sections: Vec<_> = pe
            .sections
            .iter()
            .filter(|section| section.name().ok() == Some(reserved))
            .collect();
        if sections.len() != 1 {
            return Err(invalid("DTB fill section is missing or ambiguous"));
        }
        if after.len() as u64 > u64::from(sections[0].virtual_size) {
            return Err(invalid(
                "transformed DTB exceeds firmware's reserved capacity",
            ));
        }
    }
    if targets.is_empty() {
        return Err(invalid(
            "DTB editing requires a recognized PMI measurement contract",
        ));
    }
    Ok(())
}

fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    let mut input = bytes;
    let result = ciborium::from_reader(&mut input).map_err(invalid)?;
    if input.iter().any(|byte| *byte != 0) {
        return Err(invalid("nonzero trailing PMI manifest data"));
    }
    Ok(result)
}

fn field<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    value
        .as_map()?
        .iter()
        .find(|(key, _)| key.as_text() == Some(name))
        .map(|(_, value)| value)
}

fn unique_maps(value: &Value) -> Result<(), Error> {
    match value {
        Value::Map(entries) => {
            let mut keys = std::collections::BTreeSet::new();
            for (key, value) in entries {
                let key = key
                    .as_text()
                    .ok_or_else(|| invalid("PMI manifest key is not text"))?;
                if !keys.insert(key) {
                    return Err(invalid("duplicate PMI manifest key"));
                }
                unique_maps(value)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                unique_maps(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn invalid(error: impl std::fmt::Display) -> Error {
    Error::Invalid(format!("firmware/DTB compatibility: {error}"))
}
