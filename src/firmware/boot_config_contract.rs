// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{FirmwareError, Manifest, ManifestValue};
use ciborium::Value;
use std::collections::BTreeSet;

/// Minimum native bootconfig capacity accepted by every declared PMI target.
#[derive(Debug)]
pub(crate) struct BootConfigContract {
    capacity: u32,
}

impl BootConfigContract {
    pub(crate) fn from_pmi(bytes: &[u8], architecture: &str) -> Result<Self, FirmwareError> {
        let pe = goblin::pe::PE::parse(bytes).map_err(FirmwareError::invalid)?;
        let machine = pe.header.coff_header.machine;
        if !matches!(
            (machine, architecture),
            (0x8664, "amd64") | (0xaa64, "arm64")
        ) {
            return Err(FirmwareError::invalid(
                "PMI machine does not match image platform",
            ));
        }
        let mut names = BTreeSet::new();
        for section in &pe.sections {
            if !names.insert(section.name().map_err(FirmwareError::invalid)?) {
                return Err(FirmwareError::invalid("duplicate PE section"));
            }
        }
        let mut capacities = Vec::new();
        for section in &pe.sections {
            let name = section.name().map_err(FirmwareError::invalid)?;
            if !name.starts_with(".pmi.") {
                continue;
            }
            let start = section.pointer_to_raw_data as usize;
            let length = section.size_of_raw_data as usize;
            if length > 64 * 1024 {
                return Err(FirmwareError::invalid("PMI target metadata exceeds 64 KiB"));
            }
            let end = start
                .checked_add(length)
                .ok_or_else(|| FirmwareError::invalid("PMI extent overflow"))?;
            let manifest = Manifest(
                bytes
                    .get(start..end)
                    .ok_or_else(|| FirmwareError::invalid("PMI target exceeds file"))?,
            );
            let value: Value = manifest.decode()?;
            ManifestValue(&value).unique_maps()?;
            match name {
                ".pmi.vm" if machine == 0x8664 => {
                    manifest.decode::<pmi::vm::Spec<pmi::vm::vcpu::x86_64::CpuState>>()?;
                }
                ".pmi.vm" => {
                    manifest.decode::<pmi::vm::Spec<pmi::vm::vcpu::aarch64::CpuState>>()?;
                }
                ".pmi.sev" => {
                    if manifest.decode::<pmi::sev::Spec>()?.id.is_some() {
                        return Err(FirmwareError::invalid(
                            "external boot configuration cannot preserve a signed SEV launch identity",
                        ));
                    }
                }
                ".pmi.tdx" => {
                    manifest.decode::<pmi::tdx::Spec>()?;
                }
                ".pmi.cca" => {
                    manifest.decode::<pmi::cca::Spec>()?;
                }
                _ => return Err(FirmwareError::invalid("unrecognized PMI target")),
            }
            let value = ManifestValue(&value);
            let actions = value
                .field("actions")
                .and_then(Value::as_array)
                .ok_or_else(|| FirmwareError::invalid("PMI has no actions"))?;
            let mut capacity = None;
            let mut ranges = Vec::new();
            for action in actions {
                let action = ManifestValue(action);
                let section = action
                    .field("section")
                    .and_then(Value::as_text)
                    .and_then(|name| pe.sections.iter().find(|s| s.name().ok() == Some(name)))
                    .ok_or_else(|| FirmwareError::invalid("action references missing section"))?;
                let gpa: u64 = action
                    .field("gpa")
                    .and_then(Value::as_integer)
                    .and_then(|value| value.try_into().ok())
                    .ok_or_else(|| FirmwareError::invalid("action lacks valid address"))?;
                let end = gpa
                    .checked_add(u64::from(section.virtual_size).next_multiple_of(4096))
                    .ok_or_else(|| FirmwareError::invalid("action address overflow"))?;
                if !gpa.is_multiple_of(4096)
                    || section.virtual_size == 0
                    || ranges
                        .iter()
                        .any(|(start, stop)| gpa < *stop && *start < end)
                {
                    return Err(FirmwareError::invalid(
                        "unaligned, empty, or overlapping action range",
                    ));
                }
                ranges.push((gpa, end));
                if action.field("type").and_then(Value::as_text) == Some("fill")
                    && action.field("kind").and_then(Value::as_text) == Some("lbc:bootconfig")
                {
                    if capacity.is_some()
                        || section.size_of_raw_data != 0
                        || section.characteristics & 0x80 == 0
                        || section.characteristics & 0x0200_0000 != 0
                        || !section.virtual_size.is_multiple_of(4)
                    {
                        return Err(FirmwareError::invalid(
                            "bootconfig requires one nonempty, aligned Zero reservation",
                        ));
                    }
                    capacity = Some(section.virtual_size);
                }
            }
            capacities.push(capacity.ok_or_else(|| {
                FirmwareError::invalid("PMI target has no bootconfig reservation")
            })?);
        }
        let capacity = capacities
            .into_iter()
            .min()
            .ok_or_else(|| FirmwareError::invalid("PMI has no recognized targets"))?;
        Ok(Self { capacity })
    }

    pub(crate) fn check_size(&self, text_size: usize) -> Result<(), FirmwareError> {
        let size = text_size
            .checked_add(4)
            .map(|size| size & !3)
            .and_then(|size| size.checked_add(20));
        if text_size != 0 && size.is_none_or(|size| size > self.capacity as usize) {
            return Err(FirmwareError::invalid(
                "bootconfig text and native footer exceed PMI reservation",
            ));
        }
        Ok(())
    }
}
