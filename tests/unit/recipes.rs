// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::{
    document::YamlDocument,
    recipe::{Directive, Recipe, TransformSelection},
};

#[derive(Debug)]
struct Fixture;
impl Fixture {
    fn parse(text: &str) -> Result<Recipe, conglobate::recipe::RecipeError> {
        Recipe::try_from(YamlDocument::from(text))
    }
    fn bootstrap() -> &'static str {
        "bootstrap:\n  from: tools\n  filesystem: {type: ext4, size: 4GiB}\n"
    }
}

#[test]
fn all_pipeline_combinations_obey_input_rules() {
    for bits in 0..8 {
        let mut text = String::new();
        if bits & 1 != 0 {
            text.push_str(Fixture::bootstrap());
        }
        if bits & 2 != 0 {
            text.push_str(if bits & 1 != 0 {
                "carapace: {}\n"
            } else {
                "carapace: {from: base}\n"
            });
        }
        if bits & 4 != 0 {
            text.push_str("firmware:\n  into: {pmi: /out/boot.pmi, dtb: /out/base.dtb}\n");
            if bits & 3 == 0 {
                text.push_str("  from: tools\n");
            }
        }
        assert_eq!(Fixture::parse(&text).is_ok(), bits != 0, "{bits}: {text}");
    }
    assert!(Fixture::parse("carapace: {}").is_err());
    assert!(Fixture::parse(&(Fixture::bootstrap().to_owned() + "carapace: {from: base}")).is_err());
    assert!(Fixture::parse("firmware: {into: {pmi: /a, dtb: /b}}").is_err());
}

#[test]
fn omitted_empty_and_null_have_distinct_meanings() {
    let builtin = Fixture::parse("carapace: {from: base}").unwrap();
    assert!(matches!(builtin.transform(), TransformSelection::Builtin));
    let explicit = Fixture::parse("transform: []\ncarapace: {from: base}").unwrap();
    assert!(matches!(explicit.transform(),TransformSelection::Authored(v) if v.is_empty()));
    for section in ["bootstrap", "carapace", "firmware", "transform"] {
        let input = Fixture::bootstrap().to_owned() + &format!("{section}: null\n");
        assert!(Fixture::parse(&input).is_err(), "{input}");
    }
}

#[test]
fn run_defaults_and_directive_exclusivity() {
    let recipe=Fixture::parse("carapace:\n  from: base\n  derive:\n    - run: echo ready\n    - run: echo network\n      network: true\n      timeout: 30m\n").unwrap();
    let directives = recipe.carapace().unwrap().directives();
    let Directive::Run(first) = &directives[0] else {
        panic!()
    };
    assert!(!first.network());
    assert_eq!(first.timeout().duration().as_secs(), 300);
    let Directive::Run(second) = &directives[1] else {
        panic!()
    };
    assert!(second.network());
    assert_eq!(second.timeout().duration().as_secs(), 1800);
    for bad in [
        "run: echo ok, copy: {from: a, into: /b}",
        "copy: {from: a, into: /b}, network: false",
        "run: echo ok, timeout: 0s",
        "run: echo ok, network: null",
        "run: echo ok, extra: true",
    ] {
        assert!(
            Fixture::parse(&format!("carapace: {{from: base, derive: [{{{bad}}}]}}")).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn duplicate_unknown_sections_and_invalid_firmware_are_rejected() {
    for text in [
        "carapace: {from: a}\ncarapace: {from: b}",
        "carapace: {from: a, from: b}",
        "carapace: {from: a}\nplatform: {}",
        "carapace: {from: a}\nunknown: {}",
        "firmware: {from: tools, into: {pmi: /same, dtb: /same}}",
        "firmware: {from: tools, into: {pmi: /a}}",
    ] {
        assert!(Fixture::parse(text).is_err(), "{text}");
    }
}

#[test]
fn typed_transform_values_are_checked_before_execution() {
    let valid = "carapace: {from: base}\ntransform:\n - {op: set, property: /chosen/n, type: u32, data: 4294967295}\n - {op: set, property: /flag, type: empty}\n - {op: regex, property: /chosen/bootargs, match: 'quiet', replace: '', if: {exists: /chosen/bootargs}}";
    assert!(Fixture::parse(valid).is_ok());
    for operation in [
        "op: set, property: /x, type: bytes, data: '01 zz'",
        "op: set, property: /x, type: bytes, data: '${CARAPACE}'",
        "op: set, property: /x, type: u32, data: 4294967296",
        "op: set, property: /x, type: empty, data: null",
        "op: set, property: /x, type: strings, data: [1]",
        "op: regex, property: /x, match: '[', replace: x",
        "op: remove, property: /",
        "op: remove, property: /x, data: 1",
    ] {
        assert!(
            Fixture::parse(&format!(
                "carapace: {{from: base}}\ntransform: [{{{operation}}}]"
            ))
            .is_err(),
            "{operation}"
        );
    }
}

#[test]
fn null_nested_mappings_and_directive_lists_are_not_empty_values() {
    for recipe in [
        "bootstrap: {from: tools, filesystem: {type: ext4, size: 16MiB, properties: null}}",
        "bootstrap: {from: tools, filesystem: {type: ext4, size: 16MiB, properties: {label: null}}}",
        "carapace: {from: base, derive: null}",
        "firmware: {from: tools, derive: null, into: {pmi: /pmi, dtb: /dtb}}",
    ] {
        assert!(Fixture::parse(recipe).is_err(), "{recipe}");
    }
}
