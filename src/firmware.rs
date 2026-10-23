// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Firmware permission to substitute a final measured external DTB.
use ciborium::Value;
use std::collections::BTreeSet;

#[derive(Debug, thiserror::Error)]
#[error("firmware/DTB compatibility: {0}")]
pub struct FirmwareError(String);
impl FirmwareError {
    fn invalid(error: impl std::fmt::Display) -> Self {
        Self(error.to_string())
    }
}
/// Every recognized target accepts one external measured DTB, without an
/// already-signed launch identity. This is not validation of executable firmware
/// or a signing operation. Unchanged DTBs do not need this permission.
#[derive(Debug)]
pub struct DtbContract {
    capacity: u32,
}
impl DtbContract {
    pub fn from_pmi(bytes: &[u8]) -> Result<Self, FirmwareError> {
        let pe = goblin::pe::PE::parse(bytes).map_err(FirmwareError::invalid)?;
        let mut targets = BTreeSet::new();
        let mut capacity = u32::MAX;
        for section in &pe.sections {
            let name = section.name().map_err(FirmwareError::invalid)?;
            if !name.starts_with(".pmi.") {
                continue;
            }
            if !targets.insert(name) {
                return Err(FirmwareError::invalid("duplicate PMI target section"));
            }
            let start = section.pointer_to_raw_data as usize;
            let end = start
                .checked_add(section.size_of_raw_data as usize)
                .ok_or_else(|| FirmwareError::invalid("PMI section extent overflow"))?;
            let manifest = Manifest(
                bytes
                    .get(start..end)
                    .ok_or_else(|| FirmwareError::invalid("PMI section exceeds file"))?,
            );
            let value: Value = manifest.decode()?;
            ManifestValue(&value).unique_maps()?;
            match name {
                ".pmi.vm" => match pe.header.coff_header.machine {
                    0x8664 => {
                        manifest.decode::<pmi::vm::Spec<pmi::vm::vcpu::x86_64::CpuState>>()?;
                    }
                    0xaa64 => {
                        manifest.decode::<pmi::vm::Spec<pmi::vm::vcpu::aarch64::CpuState>>()?;
                    }
                    _ => return Err(FirmwareError::invalid("unsupported VM machine")),
                },
                ".pmi.sev" => {
                    if manifest.decode::<pmi::sev::Spec>()?.id.is_some() {
                        return Err(FirmwareError::invalid(
                            "changed DTB would invalidate signed SEV launch identity",
                        ));
                    }
                }
                ".pmi.tdx" => {
                    manifest.decode::<pmi::tdx::Spec>()?;
                }
                ".pmi.cca" => {
                    manifest.decode::<pmi::cca::Spec>()?;
                }
                _ => {
                    return Err(FirmwareError::invalid(format!(
                        "DTB edits unsupported for target {name}"
                    )));
                }
            }
            let value = ManifestValue(&value);
            let actions = value
                .field("actions")
                .and_then(Value::as_array)
                .ok_or_else(|| FirmwareError::invalid("PMI has no actions"))?;
            let fills: Vec<_> = actions
                .iter()
                .map(ManifestValue)
                .filter(|action| {
                    action.field("type").and_then(Value::as_text) == Some("fill")
                        && action.field("kind").and_then(Value::as_text) == Some("dt:dtb")
                })
                .collect();
            if fills.len() != 1 {
                return Err(FirmwareError::invalid(
                    "target must accept exactly one external measured DTB",
                ));
            }
            let reserved = fills[0]
                .field("section")
                .and_then(Value::as_text)
                .ok_or_else(|| FirmwareError::invalid("DTB fill lacks section"))?;
            let mut sections = pe
                .sections
                .iter()
                .filter(|s| s.name().ok() == Some(reserved));
            let section = sections
                .next()
                .ok_or_else(|| FirmwareError::invalid("missing DTB fill section"))?;
            if sections.next().is_some() {
                return Err(FirmwareError::invalid("ambiguous DTB fill section"));
            }
            if section.size_of_raw_data != 0 || section.characteristics & 0x80 == 0 {
                return Err(FirmwareError::invalid(
                    "DTB fill must refer to a Zero section",
                ));
            }
            capacity = capacity.min(section.virtual_size);
        }
        if targets.is_empty() {
            return Err(FirmwareError::invalid(
                "changed DTB requires a recognized PMI measurement contract",
            ));
        }
        Ok(Self { capacity })
    }
    pub fn check_size(&self, bytes: usize) -> Result<(), FirmwareError> {
        if bytes > self.capacity as usize {
            Err(FirmwareError::invalid(
                "transformed DTB exceeds firmware reserved capacity",
            ))
        } else {
            Ok(())
        }
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
