// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{FirmwareError, Manifest, ManifestValue};
use ciborium::Value;
use std::collections::BTreeSet;

/// Structural launch validation independent of external boot configuration.
#[derive(Debug)]
pub(crate) struct PmiContract;
impl PmiContract {
    pub(crate) fn validate(bytes: &[u8], architecture: &str) -> Result<(), FirmwareError> {
        let pe = goblin::pe::PE::parse(bytes).map_err(FirmwareError::invalid)?;
        let machine = pe.header.coff_header.machine;
        if !pe.is_64
            || !matches!(
                (machine, architecture),
                (0x8664, "amd64") | (0xaa64, "arm64")
            )
        {
            return Err(FirmwareError::invalid(
                "PMI machine does not match image platform",
            ));
        }
        let mut names = BTreeSet::new();
        let mut raw_ranges = Vec::new();
        for section in &pe.sections {
            let name = section.name().map_err(FirmwareError::invalid)?;
            if name.is_empty() || !names.insert(name) {
                return Err(FirmwareError::invalid("empty or duplicate PE section name"));
            }
            let start = section.pointer_to_raw_data as u64;
            let end = start + section.size_of_raw_data as u64;
            if section.size_of_raw_data > 0 {
                if end > bytes.len() as u64
                    || raw_ranges
                        .iter()
                        .any(|&(first, last)| start < last && first < end)
                {
                    return Err(FirmwareError::invalid(
                        "overlapping or truncated PE section data",
                    ));
                }
                raw_ranges.push((start, end));
            }
        }
        let mut targets = 0;
        for section in &pe.sections {
            let name = section.name().map_err(FirmwareError::invalid)?;
            if !name.starts_with(".pmi.") {
                continue;
            }
            targets += 1;
            if section.characteristics & 0x0200_0000 == 0
                || section.size_of_raw_data == 0
                || section.size_of_raw_data > 65536
            {
                return Err(FirmwareError::invalid(
                    "PMI target metadata must be non-loaded and at most 64 KiB",
                ));
            }
            let start = section.pointer_to_raw_data as usize;
            let manifest = Manifest(&bytes[start..start + section.size_of_raw_data as usize]);
            let value: Value = manifest.decode()?;
            ManifestValue(&value).unique_maps()?;
            match (name, machine) {
                (".pmi.vm", 0x8664) => {
                    manifest.decode::<pmi::vm::Spec<pmi::vm::vcpu::x86_64::CpuState>>()?;
                }
                (".pmi.vm", 0xaa64) => {
                    manifest.decode::<pmi::vm::Spec<pmi::vm::vcpu::aarch64::CpuState>>()?;
                }
                (".pmi.sev", 0x8664) => {
                    manifest.decode::<pmi::sev::Spec>()?;
                }
                (".pmi.tdx", 0x8664) => {
                    manifest.decode::<pmi::tdx::Spec>()?;
                }
                (".pmi.cca", 0xaa64) => {
                    manifest.decode::<pmi::cca::Spec>()?;
                }
                _ => return Err(FirmwareError::invalid("unsupported PMI target for machine")),
            }
            let value = ManifestValue(&value);
            let actions = value
                .field("actions")
                .and_then(Value::as_array)
                .filter(|actions| !actions.is_empty())
                .ok_or_else(|| FirmwareError::invalid("PMI target requires actions"))?;
            let mut ranges = Vec::new();
            for action in actions {
                let action = ManifestValue(action);
                let section = action
                    .field("section")
                    .and_then(Value::as_text)
                    .and_then(|name| {
                        pe.sections
                            .iter()
                            .find(|section| section.name().ok() == Some(name))
                    })
                    .ok_or_else(|| FirmwareError::invalid("action references missing section"))?;
                let gpa: u64 = action
                    .field("gpa")
                    .and_then(Value::as_integer)
                    .and_then(|number| number.try_into().ok())
                    .ok_or_else(|| FirmwareError::invalid("action has invalid address"))?;
                let granularity = if section.virtual_size >= 2 * 1024 * 1024 {
                    2 * 1024 * 1024
                } else {
                    4096
                };
                let end = gpa
                    .checked_add(u64::from(section.virtual_size).next_multiple_of(4096))
                    .ok_or_else(|| FirmwareError::invalid("action address overflow"))?;
                if section.virtual_size == 0
                    || section.size_of_raw_data > section.virtual_size
                    || !gpa.is_multiple_of(granularity)
                    || !u64::from(section.pointer_to_raw_data).is_multiple_of(granularity)
                    || ranges
                        .iter()
                        .any(|&(first, last)| gpa < last && first < end)
                {
                    return Err(FirmwareError::invalid(
                        "invalid section shape, alignment or overlapping action range",
                    ));
                }
                ranges.push((gpa, end));
                if action.field("type").and_then(Value::as_text) == Some("fill") {
                    if section.size_of_raw_data != 0 || section.characteristics & 0x80 == 0 {
                        return Err(FirmwareError::invalid("fill requires a Zero section"));
                    }
                    if action.field("kind").and_then(Value::as_text) == Some("dt:dtb") {
                        return Err(FirmwareError::invalid(
                            "PMI requires an embedded base DTB; detached DTB fills are unsupported",
                        ));
                    }
                }
            }
            if let Some(base) = value.field("dt:dtb").and_then(Value::as_text) {
                let loaded = actions.iter().map(ManifestValue).any(|action| {
                    action.field("type").and_then(Value::as_text) == Some("load")
                        && action.field("section").and_then(Value::as_text) == Some(base)
                });
                if !loaded {
                    return Err(FirmwareError::invalid("embedded base DTB is not loaded"));
                }
            }
        }
        if targets == 0 {
            return Err(FirmwareError::invalid("PMI has no launch targets"));
        }
        Ok(())
    }
}
