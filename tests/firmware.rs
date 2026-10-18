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

#[test]
fn every_target_must_accept_the_dtb_and_fill_sections_must_be_unambiguous() {
    let vm = fixture::detached();
    let mut tdx = vm.clone();
    tdx.as_object_mut().unwrap().remove("vm:vcpu");
    let valid = fixture::sections(&[(".pmi.vm", &vm), (".pmi.tdx", &tdx)], &[(".dtb", 4096)]);
    validate_transform(&valid, b"before", b"after").unwrap();
    let mut invalid = tdx.clone();
    invalid["actions"] = serde_json::json!([]);
    for targets in [
        vec![(".pmi.vm", &vm), (".pmi.tdx", &invalid)],
        vec![(".pmi.tdx", &invalid), (".pmi.vm", &vm)],
    ] {
        assert!(
            validate_transform(
                &fixture::sections(&targets, &[(".dtb", 4096)]),
                b"before",
                b"after"
            )
            .is_err()
        );
    }
    let duplicate = fixture::sections(&[(".pmi.vm", &vm), (".pmi.vm", &vm)], &[(".dtb", 4096)]);
    assert!(
        validate_transform(&duplicate, b"before", b"after")
            .unwrap_err()
            .to_string()
            .contains("duplicate PMI target")
    );
    for reserves in [vec![], vec![(".dtb", 4096), (".dtb", 4096)]] {
        let image = fixture::sections(&[(".pmi.vm", &vm)], &reserves);
        assert!(
            validate_transform(&image, b"before", b"after")
                .unwrap_err()
                .to_string()
                .contains("missing or ambiguous")
        );
    }
    let mut cca = tdx.clone();
    cca["cca:vcpu"] = serde_json::json!({"pstate": 5});
    cca["cpu:profile"] = "armv8.2-a".into();
    for (target, mut spec) in [(".pmi.tdx", tdx), (".pmi.cca", cca)] {
        validate_transform(&fixture::image(target, &spec, 4096), b"before", b"after").unwrap();
        // Each target must be decoded against its own strict schema.
        spec["unexpected"] = true.into();
        assert!(
            validate_transform(&fixture::image(target, &spec, 4096), b"before", b"after").is_err()
        );
    }
}
