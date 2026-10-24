// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::*;
/// Independent small PE/CBOR encoder; fixtures are structural, not bootable.
#[derive(Debug)]
pub(crate) struct Fixture(Vec<u8>);
impl Fixture {
    pub(crate) fn detached() -> serde_json::Value {
        serde_json::json!({"version":1,"vm:vcpu":{},"cpu:profile":"x86-64-v2","actions":[{"type":"fill","gpa":8192,"section":".dtb","kind":"dt:dtb"}]})
    }
    pub(crate) fn image(target: &str, spec: &serde_json::Value, capacity: u32) -> Vec<u8> {
        Self::sections(0x8664, &[(target, spec)], &[(".dtb", capacity)])
    }
    fn sections(
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
fn vm_machine_selects_architecture_specific_cpu_validation() {
    let x86 = Fixture::detached();
    let mut arm = x86.clone();
    arm["cpu:profile"] = "armv8.2-a".into();
    for pstate in [4, 5] {
        arm["vm:vcpu"] = serde_json::json!({"pstate": pstate, "pc": 8192});
        let image = Fixture::sections(0xaa64, &[(".pmi.vm", &arm)], &[(".dtb", 4096)]);
        let contract = DtbContract::from_pmi(&image).unwrap();
        contract.check_size(4096).unwrap();
        assert!(contract.check_size(4097).is_err());
        // An x86 machine must not decode an ARM register map successfully.
        assert!(DtbContract::from_pmi(&Fixture::image(".pmi.vm", &arm, 4096)).is_err());
    }
    for state in [
        serde_json::json!({}),
        serde_json::json!({"pstate": 0}),
        serde_json::json!({"pstate": 9}),
        serde_json::json!({"pstate": 5, "rax": 1}),
    ] {
        arm["vm:vcpu"] = state;
        assert!(
            DtbContract::from_pmi(&Fixture::sections(
                0xaa64,
                &[(".pmi.vm", &arm)],
                &[(".dtb", 4096)],
            ))
            .is_err()
        );
    }
    assert!(
        DtbContract::from_pmi(&Fixture::sections(
            0xaa64,
            &[(".pmi.vm", &x86)],
            &[(".dtb", 4096)],
        ))
        .is_err()
    );
    assert!(
        DtbContract::from_pmi(&Fixture::sections(
            0x14c,
            &[(".pmi.vm", &x86)],
            &[(".dtb", 4096)],
        ))
        .unwrap_err()
        .to_string()
        .contains("unsupported VM machine")
    );
}
#[test]
fn every_target_must_accept_external_dtb_and_capacity() {
    let vm = Fixture::detached();
    let mut tdx = vm.clone();
    tdx.as_object_mut().unwrap().remove("vm:vcpu");
    let mut cca = tdx.clone();
    cca["cca:vcpu"] = serde_json::json!({"pstate":5});
    cca["cpu:profile"] = "armv8.2-a".into();
    for name in [".pmi.vm", ".pmi.sev", ".pmi.tdx", ".pmi.cca"] {
        let spec = match name {
            ".pmi.vm" => &vm,
            ".pmi.cca" => &cca,
            _ => &tdx,
        };
        let image = Fixture::image(name, spec, 4096);
        let contract = DtbContract::from_pmi(&image).unwrap();
        contract.check_size(4096).unwrap();
        assert!(contract.check_size(4097).is_err());
    }
    let mut optional = vm.clone();
    optional["dt:dtb"] = ".dtb".into();
    DtbContract::from_pmi(&Fixture::image(".pmi.vm", &optional, 4096)).unwrap();
    let mut smaller = tdx.clone();
    smaller["actions"][0]["section"] = ".small".into();
    let mixed = DtbContract::from_pmi(&Fixture::sections(
        0x8664,
        &[(".pmi.vm", &vm), (".pmi.tdx", &smaller)],
        &[(".dtb", 4096), (".small", 128)],
    ))
    .unwrap();
    mixed.check_size(128).unwrap();
    assert!(mixed.check_size(129).is_err());
    let mut no_fill = tdx.clone();
    no_fill["actions"] = serde_json::json!([]);
    for targets in [
        vec![(".pmi.vm", &vm), (".pmi.tdx", &no_fill)],
        vec![(".pmi.tdx", &no_fill), (".pmi.vm", &vm)],
    ] {
        assert!(
            DtbContract::from_pmi(&Fixture::sections(0x8664, &targets, &[(".dtb", 4096)])).is_err()
        );
    }
    let mut signed = tdx;
    signed["sev:id"] = serde_json::json!({"block":".id","auth":".auth"});
    assert!(
        DtbContract::from_pmi(&Fixture::image(".pmi.sev", &signed, 4096))
            .unwrap_err()
            .to_string()
            .contains("signed SEV")
    );
}
#[test]
fn ambiguous_unknown_malformed_and_nonzero_fill_sections_fail() {
    let vm = Fixture::detached();
    for targets in [
        vec![(".pmi.vm", &vm), (".pmi.vm", &vm)],
        vec![(".pmi.bad", &vm)],
    ] {
        assert!(
            DtbContract::from_pmi(&Fixture::sections(0x8664, &targets, &[(".dtb", 4096)])).is_err()
        );
    }
    for sections in [vec![], vec![(".dtb", 4096), (".dtb", 4096)]] {
        assert!(
            DtbContract::from_pmi(&Fixture::sections(0x8664, &[(".pmi.vm", &vm)], &sections))
                .is_err()
        );
    }
    let mut duplicate_fill = vm.clone();
    duplicate_fill["actions"]
        .as_array_mut()
        .unwrap()
        .push(vm["actions"][0].clone());
    assert!(DtbContract::from_pmi(&Fixture::image(".pmi.vm", &duplicate_fill, 4096)).is_err());
    let mut unknown = vm.clone();
    unknown["unexpected"] = true.into();
    assert!(DtbContract::from_pmi(&Fixture::image(".pmi.vm", &unknown, 4096)).is_err());
    let mut bytes = Fixture::image(".pmi.vm", &vm, 4096);
    let last = bytes.len() - 1;
    bytes[last] = 1;
    assert!(DtbContract::from_pmi(&bytes).is_err());
    let mut image = Fixture(Fixture::image(".pmi.vm", &vm, 4096));
    image.u32(0x98 + 240 + 40 + 36, 0x40000040);
    assert!(DtbContract::from_pmi(&image.0).is_err());
    assert!(DtbContract::from_pmi(b"opaque firmware").is_err());
    let mut duplicate = Value::serialized(&vm).unwrap();
    duplicate
        .as_map_mut()
        .unwrap()
        .push((Value::Text("version".into()), Value::Integer(1.into())));
    let mut encoded = Vec::new();
    ciborium::into_writer(&duplicate, &mut encoded).unwrap();
    let mut image = Fixture(Fixture::image(".pmi.vm", &vm, 4096));
    image.0[512..].fill(0);
    image.0[512..512 + encoded.len()].copy_from_slice(&encoded);
    image.u32(0x98 + 240 + 8, encoded.len() as u32);
    assert!(
        DtbContract::from_pmi(&image.0)
            .unwrap_err()
            .to_string()
            .contains("duplicate manifest key")
    );
}
