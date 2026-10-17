// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

#[path = "common/firmware.rs"]
mod fixture;
use conglobate::firmware::validate_transform;

#[test]
fn detached_and_optional_channels_accept_final_dtb_bytes() {
    let mut spec = fixture::detached();
    let image = fixture::image(".pmi.vm", &spec, 4096);
    validate_transform(&image, b"before", b"after").unwrap();
    spec["dt:dtb"] = ".dtb".into();
    let image = fixture::image(".pmi.vm", &spec, 4096);
    validate_transform(&image, b"before", b"after").unwrap();
}

#[test]
fn unsupported_bound_or_oversized_edits_fail() {
    let mut spec = fixture::detached();
    let image = fixture::image(".pmi.vm", &spec, 4);
    assert!(validate_transform(&image, b"before", b"after").is_err());
    spec["dt:dtb"] = ".dtb".into();
    spec["actions"] = serde_json::json!([]);
    let image = fixture::image(".pmi.vm", &spec, 4096);
    assert!(validate_transform(&image, b"before", b"after").is_err());
    assert!(validate_transform(b"unknown firmware", b"before", b"after").is_err());
    validate_transform(b"unknown firmware", b"unchanged", b"unchanged").unwrap();
    let mut sev = fixture::detached();
    sev.as_object_mut().unwrap().remove("vm:vcpu");
    sev["sev:id"] = serde_json::json!({"block": ".id", "auth": ".auth"});
    let image = fixture::image(".pmi.sev", &sev, 4096);
    let error = validate_transform(&image, b"before", b"after").unwrap_err();
    assert!(error.to_string().contains("signed SEV"), "{error}");
}
