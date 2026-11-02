// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

mod boot_config_contract;
mod output;
mod pmi_contract;
pub(crate) use boot_config_contract::BootConfigContract;
use ciborium::Value;
pub(crate) use output::FirmwareFile;
pub(crate) use pmi_contract::PmiContract;
use std::collections::BTreeSet;

#[derive(Debug, thiserror::Error)]
#[error("firmware compatibility: {0}")]
pub(crate) struct FirmwareError(String);
impl FirmwareError {
    fn invalid(error: impl std::fmt::Display) -> Self {
        Self(error.to_string())
    }
}
#[derive(Debug)]
struct Manifest<'a>(&'a [u8]);
impl Manifest<'_> {
    fn decode<T: serde::de::DeserializeOwned>(&self) -> Result<T, FirmwareError> {
        let mut bytes = self.0;
        let value = ciborium::from_reader(&mut bytes).map_err(FirmwareError::invalid)?;
        if bytes.iter().any(|byte| *byte != 0) {
            return Err(FirmwareError::invalid("nonzero trailing manifest bytes"));
        }
        Ok(value)
    }
}
#[derive(Debug)]
struct ManifestValue<'a>(&'a Value);
impl<'a> ManifestValue<'a> {
    fn field(&self, name: &str) -> Option<&'a Value> {
        self.0
            .as_map()?
            .iter()
            .find(|(key, _)| key.as_text() == Some(name))
            .map(|(_, value)| value)
    }
    fn unique_maps(&self) -> Result<(), FirmwareError> {
        match self.0 {
            Value::Map(entries) => {
                let mut keys = BTreeSet::new();
                for (key, value) in entries {
                    let key = key
                        .as_text()
                        .ok_or_else(|| FirmwareError::invalid("manifest key is not text"))?;
                    if !keys.insert(key) {
                        return Err(FirmwareError::invalid("duplicate manifest key"));
                    }
                    Self(value).unique_maps()?;
                }
            }
            Value::Array(values) => {
                for value in values {
                    Self(value).unique_maps()?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests;
