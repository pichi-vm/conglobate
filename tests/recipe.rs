// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::recipe::{Directive, ImageLock, Pipeline, RecipeFiles};

const BOOTSTRAP: &str = "from: tools:1\ninto: /out/rootfs.raw\n";
const CARAPACE: &str = "from: base:1\n";
const FIRMWARE: &str = "into: {pmi: /out/boot.pmi, dtb: /out/base.dtb}\n";
const TOOLS_FIRMWARE: &str = "from: tools:1\ninto: {pmi: /out/boot.pmi, dtb: /out/base.dtb}\n";

#[test]
fn all_eight_stage_combinations_have_correct_input_selection() {
    for bits in 0..8 {
        let bootstrap = (bits & 1 != 0).then_some(BOOTSTRAP);
        let carapace = (bits & 2 != 0).then_some(if bootstrap.is_some() { "" } else { CARAPACE });
        let firmware = (bits & 4 != 0).then_some(if bits == 4 { TOOLS_FIRMWARE } else { FIRMWARE });
        let result = Pipeline::try_from(RecipeFiles {
            bootstrap,
            carapace,
            firmware,
            ..Default::default()
        });
        if bits == 0 {
            assert!(result.is_err());
            continue;
        }
        let pipeline = result.unwrap();
        assert_eq!(pipeline.has_carapace(), bits & 3 != 0);
        assert_eq!(pipeline.base().is_some(), bits & 3 == 2);
    }
}

#[test]
fn source_constraints_and_pair_replacement_are_enforced() {
    for reference in [
        "https://user:secret@registry.example/image",
        "user:secret@registry.example/image",
        "../image",
    ] {
        assert!(conglobate::recipe::ImageReference::try_from(reference.to_owned()).is_err());
    }
    for files in [
        RecipeFiles {
            bootstrap: Some(BOOTSTRAP),
            carapace: Some(CARAPACE),
            ..Default::default()
        },
        RecipeFiles {
            carapace: Some(""),
            ..Default::default()
        },
        RecipeFiles {
            firmware: Some(FIRMWARE),
            ..Default::default()
        },
        RecipeFiles {
            firmware: Some("from: tools:1\ninto: {pmi: /out/pmi}"),
            ..Default::default()
        },
        RecipeFiles {
            firmware: Some("from: tools:1\ninto: {pmi: /out/pmi, dtb: /out/pmi}"),
            ..Default::default()
        },
        RecipeFiles {
            carapace: Some(CARAPACE),
            platform: Some(""),
            ..Default::default()
        },
    ] {
        assert!(Pipeline::try_from(files).is_err());
    }
    let pipeline = Pipeline::try_from(RecipeFiles {
        carapace: Some(CARAPACE),
        firmware: Some(TOOLS_FIRMWARE),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(pipeline.base().unwrap().as_ref(), "base:1");
    assert_eq!(
        pipeline
            .references()
            .iter()
            .map(std::convert::AsRef::as_ref)
            .collect::<Vec<_>>(),
        ["base:1", "tools:1"]
    );
}

#[test]
fn network_is_explicit_per_run_and_never_a_copy_option() {
    let yaml = "from: base:1\nderive:\n - run: echo offline\n - run: dnf install -y kernel\n   network: true\n - copy: {from: source, into: /build/source}\n";
    let p = Pipeline::try_from(RecipeFiles {
        carapace: Some(yaml),
        ..Default::default()
    })
    .unwrap();
    let steps = &p.carapace.unwrap().derive;
    assert!(matches!(steps[0], Directive::Run { network: false, .. }));
    assert!(matches!(steps[1], Directive::Run { network: true, .. }));
    for step in [
        "{copy: {from: a, into: /a}, network: false}",
        "{run: echo, copy: {from: a, into: /a}}",
        "{run: ''}",
        "{copy: {from: [], into: /a}}",
        "{copy: {from: ../secret, into: /a}}",
    ] {
        let yaml = format!("from: base:1\nderive: [{step}]");
        assert!(
            Pipeline::try_from(RecipeFiles {
                carapace: Some(&yaml),
                ..Default::default()
            })
            .is_err()
        );
    }
}

#[test]
fn lock_pins_only_indexes_and_checks_all_execution_inputs() {
    let p = Pipeline::try_from(RecipeFiles {
        carapace: Some(CARAPACE),
        firmware: Some(TOOLS_FIRMWARE),
        ..Default::default()
    })
    .unwrap();
    let digest = format!("sha256:{}", "ab".repeat(32));
    let yaml = format!(
        "version: 1\nimages:\n  base:1: {{index: {digest}}}\n  tools:1: {{index: {digest}}}\n"
    );
    let lock = ImageLock::parse(&yaml).unwrap();
    assert!(lock.validate(&p).is_ok());
    assert!(
        ImageLock::parse("version: 1\nimages: {}")
            .unwrap()
            .validate(&p)
            .is_err()
    );
    for text in [
        yaml.replace("version: 1", "version: 2"),
        yaml.replace("index:", "manifest:"),
        yaml.replace(&digest, "sha256:bad"),
        yaml.replace("index:", "carapace: abcd, index:"),
    ] {
        assert!(ImageLock::parse(&text).is_err());
    }
}

#[test]
fn duplicate_lock_entries_are_rejected() {
    let digest = format!("sha256:{}", "ab".repeat(32));
    let yaml = format!(
        "version: 1\nimages:\n  base:1: {{index: {digest}}}\n  base:1: {{index: {digest}}}\n"
    );
    assert!(ImageLock::parse(&yaml).is_err());
}
