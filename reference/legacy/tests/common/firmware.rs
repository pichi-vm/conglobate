// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Small PE fixtures for the PMI DTB channel; these are not executable kernels.

pub(crate) fn image(target: &str, spec: &serde_json::Value, capacity: u32) -> Vec<u8> {
    sections(&[(target, spec)], &[(".dtb", capacity)])
}

/// Multiple target manifests and independently named reserved fill sections.
pub(crate) fn sections(
    targets: &[(&str, &serde_json::Value)],
    reserves: &[(&str, u32)],
) -> Vec<u8> {
    let count = targets.len() + reserves.len();
    let headers = (0x98 + 240 + count * 40).div_ceil(512) * 512;
    let mut bytes = vec![0; headers];
    bytes[..2].copy_from_slice(b"MZ");
    put32(&mut bytes, 0x3c, 0x80);
    bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
    put16(&mut bytes, 0x84, 0x8664);
    put16(&mut bytes, 0x86, count as u16);
    put16(&mut bytes, 0x94, 240);
    put16(&mut bytes, 0x96, 0x22);
    put16(&mut bytes, 0x98, 0x20b);
    put32(&mut bytes, 0x98 + 32, 4096);
    put32(&mut bytes, 0x98 + 36, 512);
    put32(&mut bytes, 0x98 + 56, 0x20000);
    put32(&mut bytes, 0x98 + 60, headers as u32);
    put16(&mut bytes, 0x98 + 68, 10);
    for (i, (target, spec)) in targets.iter().enumerate() {
        let mut cbor = Vec::new();
        ciborium::into_writer(spec, &mut cbor).unwrap();
        let raw_size = cbor.len().div_ceil(512) * 512;
        let offset = bytes.len();
        let section = 0x98 + 240 + i * 40;
        assert!(target.len() <= 8);
        bytes[section..section + target.len()].copy_from_slice(target.as_bytes());
        put32(&mut bytes, section + 8, cbor.len() as u32);
        put32(&mut bytes, section + 12, (i as u32 + 1) * 4096);
        put32(&mut bytes, section + 16, raw_size as u32);
        put32(&mut bytes, section + 20, offset as u32);
        put32(&mut bytes, section + 36, 0x4200_0040);
        bytes.resize(offset + raw_size, 0);
        bytes[offset..offset + cbor.len()].copy_from_slice(&cbor);
    }
    for (i, (name, capacity)) in reserves.iter().enumerate() {
        let section = 0x98 + 240 + (targets.len() + i) * 40;
        assert!(name.len() <= 8);
        bytes[section..section + name.len()].copy_from_slice(name.as_bytes());
        put32(&mut bytes, section + 8, *capacity);
        put32(
            &mut bytes,
            section + 12,
            (targets.len() as u32 + i as u32 + 1) * 4096,
        );
        put32(&mut bytes, section + 36, 0xc000_0080);
    }
    bytes
}

pub(crate) fn detached() -> serde_json::Value {
    serde_json::json!({"version": 1, "vm:vcpu": {}, "cpu:profile": "x86-64-v2",
        "actions": [{"type": "fill", "gpa": 0x2000, "section": ".dtb", "kind": "dt:dtb"}]})
}

fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
