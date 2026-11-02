// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::*;
/// Independent small PE/CBOR encoder; fixtures are structural, not bootable.
#[derive(Debug)]
pub(crate) struct Fixture(Vec<u8>);
impl Fixture {
    pub(crate) fn native() -> Vec<u8> {
        let machine = if cfg!(target_arch = "aarch64") {
            0xaa64
        } else {
            0x8664
        };
        Self::sections(
            machine,
            &[(".pmi.vm", &Self::bootconfig_spec(machine))],
            &[(".bconf", 4096)],
        )
    }

    pub(crate) fn detached() -> serde_json::Value {
        serde_json::json!({"version":1,"vm:vcpu":{},"cpu:profile":"x86-64-v2","actions":[{"type":"fill","gpa":8192,"section":".dtb","kind":"dt:dtb"}]})
    }
    pub(crate) fn image(target: &str, spec: &serde_json::Value, capacity: u32) -> Vec<u8> {
        Self::sections(0x8664, &[(target, spec)], &[(".dtb", capacity)])
    }
    pub(crate) fn sections(
        machine: u16,
        targets: &[(&str, &serde_json::Value)],
        reserves: &[(&str, u32)],
    ) -> Vec<u8> {
        let count = targets.len() + reserves.len();
        let headers = (0x98 + 240 + count * 40).div_ceil(512) * 512;
        let mut out = Self(vec![0; headers]);
        out.0[..2].copy_from_slice(b"MZ");
        out.u32(0x3c, 0x80);
        out.0[0x80..0x84].copy_from_slice(b"PE\0\0");
        out.u16(0x84, machine);
        out.u16(0x86, count as u16);
        out.u16(0x94, 240);
        out.u16(0x96, 0x22);
        out.u16(0x98, 0x20b);
        out.u32(0x98 + 32, 4096);
        out.u32(0x98 + 36, 512);
        out.u32(0x98 + 56, 0x20000);
        out.u32(0x98 + 60, headers as u32);
        out.u16(0x98 + 68, 10);
        for (index, (name, spec)) in targets.iter().enumerate() {
            let mut cbor = Vec::new();
            ciborium::into_writer(spec, &mut cbor).unwrap();
            let offset = out.0.len();
            let raw = cbor.len().div_ceil(512) * 512;
            let section = 0x98 + 240 + index * 40;
            out.0[section..section + name.len()].copy_from_slice(name.as_bytes());
            out.u32(section + 8, cbor.len() as u32);
            out.u32(section + 12, (index as u32 + 1) * 4096);
            out.u32(section + 16, raw as u32);
            out.u32(section + 20, offset as u32);
            out.u32(section + 36, 0x42000040);
            out.0.resize(offset + raw, 0);
            out.0[offset..offset + cbor.len()].copy_from_slice(&cbor);
        }
        for (index, (name, size)) in reserves.iter().enumerate() {
            let section = 0x98 + 240 + (targets.len() + index) * 40;
            out.0[section..section + name.len()].copy_from_slice(name.as_bytes());
            out.u32(section + 8, *size);
            out.u32(section + 12, (targets.len() + index + 1) as u32 * 4096);
            out.u32(section + 36, 0xc0000080);
        }
        out.0
    }
    fn u16(&mut self, offset: usize, value: u16) {
        self.0[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    fn u32(&mut self, offset: usize, value: u32) {
        self.0[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}
#[test]
fn pmi_targets_validate_their_cpu_architecture_without_external_dtb() {
    for (machine, architecture) in [(0x8664, "amd64"), (0xaa64, "arm64")] {
        let spec = Fixture::bootconfig_spec(machine);
        let bytes = Fixture::sections(machine, &[(".pmi.vm", &spec)], &[(".bconf", 4096)]);
        PmiContract::validate(&bytes, architecture).unwrap();
        assert!(
            PmiContract::validate(
                &bytes,
                if architecture == "amd64" {
                    "arm64"
                } else {
                    "amd64"
                }
            )
            .is_err()
        );
    }
    let mut arm = Fixture::bootconfig_spec(0xaa64);
    for state in [
        serde_json::json!({}),
        serde_json::json!({"pstate":0}),
        serde_json::json!({"pstate":9}),
        serde_json::json!({"pstate":5,"rax":1}),
    ] {
        arm["vm:vcpu"] = state;
        assert!(
            PmiContract::validate(
                &Fixture::sections(0xaa64, &[(".pmi.vm", &arm)], &[(".bconf", 4096)]),
                "arm64"
            )
            .is_err()
        );
    }
    assert!(
        PmiContract::validate(
            &Fixture::image(".pmi.vm", &Fixture::detached(), 4096),
            "amd64"
        )
        .unwrap_err()
        .to_string()
        .contains("detached DTB")
    );
    let vm = Fixture::bootconfig_spec(0x8664);
    let mut common = vm.clone();
    common.as_object_mut().unwrap().remove("vm:vcpu");
    for name in [".pmi.sev", ".pmi.tdx"] {
        let bytes = Fixture::sections(0x8664, &[(name, &common)], &[(".bconf", 4096)]);
        PmiContract::validate(&bytes, "amd64").unwrap();
    }
    common["cca:vcpu"] = serde_json::json!({"pstate":5});
    common["cpu:profile"] = "armv8.2-a".into();
    PmiContract::validate(
        &Fixture::sections(0xaa64, &[(".pmi.cca", &common)], &[(".bconf", 4096)]),
        "arm64",
    )
    .unwrap();
    assert!(
        PmiContract::validate(
            &Fixture::sections(0x8664, &[(".pmi.cca", &common)], &[(".bconf", 4096)]),
            "amd64"
        )
        .is_err()
    );
}
#[test]
fn malformed_pmi_sections_targets_and_actions_are_rejected() {
    let spec = Fixture::bootconfig_spec(0x8664);
    for targets in [
        vec![],
        vec![(".pmi.vm", &spec), (".pmi.vm", &spec)],
        vec![(".pmi.bad", &spec)],
    ] {
        assert!(
            PmiContract::validate(
                &Fixture::sections(0x8664, &targets, &[(".bconf", 4096)]),
                "amd64"
            )
            .is_err()
        );
    }
    for sections in [vec![], vec![(".bconf", 4096), (".bconf", 4096)]] {
        assert!(
            PmiContract::validate(
                &Fixture::sections(0x8664, &[(".pmi.vm", &spec)], &sections),
                "amd64"
            )
            .is_err()
        );
    }
    for changed in [
        serde_json::json!({"version":2}),
        serde_json::json!({"unexpected":true}),
        serde_json::json!({"actions":[]}),
        serde_json::json!({"actions":[spec["actions"][0].clone(),spec["actions"][0].clone()]}),
    ] {
        let mut value = spec.clone();
        value
            .as_object_mut()
            .unwrap()
            .extend(changed.as_object().unwrap().clone());
        assert!(
            PmiContract::validate(
                &Fixture::sections(0x8664, &[(".pmi.vm", &value)], &[(".bconf", 4096)]),
                "amd64"
            )
            .is_err()
        );
    }
    let valid = Fixture::sections(0x8664, &[(".pmi.vm", &spec)], &[(".bconf", 4096)]);
    for (offset, value) in [
        (0x98 + 240 + 36, 0x40000040),
        (0x98 + 240 + 40 + 36, 0x40000040),
        (0x98 + 240 + 40 + 16, 512),
        (0x98 + 240 + 20, u32::MAX),
    ] {
        let mut image = Fixture(valid.clone());
        image.u32(offset, value);
        assert!(PmiContract::validate(&image.0, "amd64").is_err());
    }
    for bytes in [b"opaque firmware".as_slice(), &valid[..valid.len() - 1]] {
        assert!(PmiContract::validate(bytes, "amd64").is_err());
    }
    let mut trailing = valid.clone();
    *trailing.last_mut().unwrap() = 1;
    assert!(PmiContract::validate(&trailing, "amd64").is_err());
    let mut duplicate = Value::serialized(&spec).unwrap();
    duplicate
        .as_map_mut()
        .unwrap()
        .push((Value::Text("version".into()), Value::Integer(1.into())));
    let mut encoded = Vec::new();
    ciborium::into_writer(&duplicate, &mut encoded).unwrap();
    let mut image = Fixture(valid);
    image.0[512..].fill(0);
    image.0[512..512 + encoded.len()].copy_from_slice(&encoded);
    image.u32(0x98 + 240 + 8, encoded.len() as u32);
    assert!(
        PmiContract::validate(&image.0, "amd64")
            .unwrap_err()
            .to_string()
            .contains("duplicate manifest key")
    );
}
#[test]
fn pmi_action_ranges_obey_granularity_and_zero_fill_rules() {
    let spec = Fixture::bootconfig_spec(0x8664);
    for (gpa, size) in [
        (8193, 4096),
        (u64::MAX - 4095, 4096),
        (8192, 2 * 1024 * 1024),
    ] {
        let mut value = spec.clone();
        value["actions"][0]["gpa"] = gpa.into();
        assert!(
            PmiContract::validate(
                &Fixture::sections(0x8664, &[(".pmi.vm", &value)], &[(".bconf", size)]),
                "amd64"
            )
            .is_err()
        );
    }
    let mut declared = spec;
    declared["dt:dtb"] = ".absent".into();
    assert!(
        PmiContract::validate(
            &Fixture::sections(0x8664, &[(".pmi.vm", &declared)], &[(".bconf", 4096)]),
            "amd64"
        )
        .is_err()
    );
}

impl Fixture {
    pub(crate) fn bootconfig_spec(machine: u16) -> serde_json::Value {
        let mut spec = Self::detached();
        spec["actions"][0]["kind"] = "lbc:bootconfig".into();
        spec["actions"][0]["section"] = ".bconf".into();
        if machine == 0xaa64 {
            spec["cpu:profile"] = "armv8.2-a".into();
            spec["vm:vcpu"] = serde_json::json!({"pstate":5,"pc":8192});
        }
        spec
    }
}

#[test]
fn bootconfig_contract_checks_architecture_and_native_footer_capacity() {
    for (machine, architecture) in [(0x8664, "amd64"), (0xaa64, "arm64")] {
        let spec = Fixture::bootconfig_spec(machine);
        let bytes = Fixture::sections(machine, &[(".pmi.vm", &spec)], &[(".bconf", 4096)]);
        let contract = BootConfigContract::from_pmi(&bytes, architecture).unwrap();
        contract.check_size(4075).unwrap();
        assert!(contract.check_size(4076).is_err());
        assert!(contract.check_size(usize::MAX).is_err());
        assert!(
            BootConfigContract::from_pmi(
                &bytes,
                if architecture == "amd64" {
                    "arm64"
                } else {
                    "amd64"
                }
            )
            .is_err()
        );
    }
}

#[test]
fn bootconfig_contract_rejects_missing_ambiguous_nonzero_and_overlapping_reservations() {
    let spec = Fixture::bootconfig_spec(0x8664);
    for (field, value) in [
        ("section", serde_json::json!(".absent")),
        ("gpa", serde_json::json!(8193)),
        ("gpa", serde_json::json!(u64::MAX - 4095)),
        ("kind", serde_json::json!("dt:dtb")),
    ] {
        let mut changed = spec.clone();
        changed["actions"][0][field] = value;
        assert!(
            BootConfigContract::from_pmi(
                &Fixture::sections(0x8664, &[(".pmi.vm", &changed)], &[(".bconf", 4096)]),
                "amd64"
            )
            .is_err()
        );
    }
    for capacity in [0, 4095] {
        assert!(
            BootConfigContract::from_pmi(
                &Fixture::sections(0x8664, &[(".pmi.vm", &spec)], &[(".bconf", capacity)]),
                "amd64"
            )
            .is_err()
        );
    }
    for gpa in [8192, 16384] {
        let mut duplicate = spec.clone();
        let mut second = spec["actions"][0].clone();
        second["gpa"] = gpa.into();
        duplicate["actions"].as_array_mut().unwrap().push(second);
        assert!(
            BootConfigContract::from_pmi(
                &Fixture::sections(0x8664, &[(".pmi.vm", &duplicate)], &[(".bconf", 4096)]),
                "amd64"
            )
            .is_err()
        );
    }
    let offset = 0x98 + 240 + 40;
    for (field, value) in [(36, 0x40000040), (36, 0xc2000080), (16, 512)] {
        let mut image = Fixture(Fixture::sections(
            0x8664,
            &[(".pmi.vm", &spec)],
            &[(".bconf", 4096)],
        ));
        image.u32(offset + field, value);
        assert!(BootConfigContract::from_pmi(&image.0, "amd64").is_err());
    }
    let mut tdx = spec.clone();
    tdx.as_object_mut().unwrap().remove("vm:vcpu");
    tdx["actions"][0]["section"] = ".small".into();
    let bytes = Fixture::sections(
        0x8664,
        &[(".pmi.vm", &spec), (".pmi.tdx", &tdx)],
        &[(".bconf", 4096), (".small", 128)],
    );
    let contract = BootConfigContract::from_pmi(&bytes, "amd64").unwrap();
    contract.check_size(107).unwrap();
    assert!(contract.check_size(108).is_err());
    tdx["actions"] = serde_json::json!([]);
    let bytes = Fixture::sections(
        0x8664,
        &[(".pmi.vm", &spec), (".pmi.tdx", &tdx)],
        &[(".bconf", 4096)],
    );
    assert!(BootConfigContract::from_pmi(&bytes, "amd64").is_err());
}
