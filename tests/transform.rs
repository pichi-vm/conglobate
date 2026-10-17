// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::transform::Transform;
use devtree::{OwnedNode, OwnedProperty, OwnedTree, Tree};

fn dtb(value: Option<Vec<u8>>) -> Vec<u8> {
    let mut chosen = OwnedNode::new("chosen");
    if let Some(bytes) = value {
        chosen.set_property(OwnedProperty::new("bootargs").with_bytes(bytes));
    }
    OwnedTree::new(
        OwnedNode::new("")
            .with_child(chosen)
            .with_property(OwnedProperty::new("untouched").with_u64(0x0001_2345_6789)),
    )
    .encode()
    .unwrap()
}
fn string(s: &str) -> Vec<u8> {
    let mut b = s.as_bytes().to_vec();
    b.push(0);
    b
}
fn owned(b: &[u8]) -> OwnedTree {
    let tree: Tree<'_> = Tree::parse(b).unwrap();
    OwnedTree::from(&tree)
}
fn apply(yaml: &str, b: &[u8], root: Option<&str>) -> Vec<u8> {
    Transform::parse(Some(yaml))
        .unwrap()
        .apply(Some(b), root)
        .unwrap()
        .unwrap()
}

#[test]
fn default_refreshes_existing_bindings_only() {
    let t = Transform::parse(None).unwrap();
    for input in [
        "corium=old",
        "quiet corium=old",
        "corium=old corium=older console=hvc0",
    ] {
        let out = t
            .apply(Some(&dtb(Some(string(input)))), Some("abcd"))
            .unwrap()
            .unwrap();
        let tree = owned(&out);
        let args = tree
            .find_path("/chosen")
            .unwrap()
            .property("bootargs")
            .unwrap()
            .as_str()
            .unwrap();
        assert!(!args.contains("old"));
        assert!(args.contains("corium=abcd"));
    }
    for input in ["", "quiet", "other.corium=keep corium.log=info"] {
        let before = dtb(Some(string(input)));
        assert_eq!(t.apply(Some(&before), None).unwrap().unwrap(), before);
    }
    assert!(
        t.apply(Some(&dtb(Some(string("corium=old")))), None)
            .is_err()
    );
}

#[test]
fn default_absence_and_authored_empty_are_different() {
    let default = Transform::parse(None).unwrap();
    assert!(default.apply(None, None).unwrap().is_none());
    let before = dtb(None);
    assert_eq!(default.apply(Some(&before), None).unwrap().unwrap(), before);
    let no_chosen = OwnedTree::new(OwnedNode::new("")).encode().unwrap();
    assert_eq!(
        default.apply(Some(&no_chosen), None).unwrap().unwrap(),
        no_chosen
    );
    for text in ["", "# empty\n", "[]"] {
        let t = Transform::parse(Some(text)).unwrap();
        assert!(t.apply(None, None).is_err());
        let before = dtb(Some(string("corium=old")));
        assert_eq!(t.apply(Some(&before), None).unwrap().unwrap(), before);
    }
}

#[test]
fn missing_reads_fail_but_exists_skips_before_reading() {
    let unconditional = "- op: regex\n  property: /chosen/bootargs\n  match: x\n  replace: y\n";
    assert!(
        Transform::parse(Some(unconditional))
            .unwrap()
            .apply(Some(&dtb(None)), None)
            .is_err()
    );
    let conditional = "- op: regex\n  if: {exists: /other/property}\n  property: /missing/target\n  match: x\n  replace: '${CARAPACE}'\n";
    let t = Transform::parse(Some(conditional)).unwrap();
    assert!(t.apply(Some(&dtb(None)), None).is_ok());
    assert!(t.apply(None, None).is_err());
}

#[test]
fn wrong_types_fail_even_with_no_matches_or_default() {
    for bytes in [
        vec![],
        vec![0, 0],
        vec![0xff, 0],
        b"unterminated".to_vec(),
        b"one\0two\0".to_vec(),
    ] {
        let before = dtb(Some(bytes));
        assert!(
            Transform::parse(None)
                .unwrap()
                .apply(Some(&before), None)
                .is_err()
        );
    }
}

#[test]
fn all_typed_values_round_trip_and_operations_are_ordered() {
    let yaml = r"
- {op: set, property: /new/text, type: string, data: '${CARAPACE} $$ $${CARAPACE}'}
- {op: set, property: /new/list, type: strings, data: [hello, '${CARAPACE}']}
- {op: set, property: /new/n32, type: u32, data: 0x12345678}
- {op: set, property: /new/n64, type: u64, data: 0x123456789abcdef0}
- {op: set, property: /new/cells, type: cells, data: [0x0, 0x80000000, 0, 0x40000000]}
- {op: set, property: /new/empty, type: empty}
- {op: set, property: /new/bytes, type: bytes, data: '01 ab 00 ff'}
- {op: set, property: /new/temporary, type: u32, data: 1}
- {op: remove, property: /new/temporary}
- {op: regex, property: /new/text, match: '^abcd', replace: 'root'}
";
    let before = dtb(None);
    let out = apply(yaml, &before, Some("abcd"));
    let tree = owned(&out);
    let node = tree.find_path("/new").unwrap();
    assert_eq!(
        node.property("text").unwrap().as_str(),
        Some("root $ ${CARAPACE}")
    );
    assert_eq!(
        node.property("list")
            .unwrap()
            .as_strs()
            .unwrap()
            .collect::<Vec<_>>(),
        ["hello", "abcd"]
    );
    assert_eq!(
        node.property("n32").unwrap().as_ref(),
        [0x12, 0x34, 0x56, 0x78]
    );
    assert_eq!(
        node.property("n64").unwrap().as_u64(),
        Some(0x1234_5678_9abc_def0)
    );
    assert_eq!(
        node.property("cells")
            .unwrap()
            .as_u32s()
            .unwrap()
            .collect::<Vec<_>>(),
        [0, 0x8000_0000, 0, 0x4000_0000]
    );
    assert_eq!(node.property("empty").unwrap().as_ref(), b"");
    assert_eq!(node.property("bytes").unwrap().as_ref(), [1, 0xab, 0, 0xff]);
    assert!(node.property("temporary").is_none());
    assert_eq!(
        tree.root().property("untouched"),
        owned(&before).root().property("untouched")
    );
}

#[test]
fn regex_is_global_and_variables_are_lazy() {
    let before = dtb(Some(string("one one")));
    let out = apply(
        "- {op: regex, property: /chosen/bootargs, match: one, replace: two}",
        &before,
        None,
    );
    assert_eq!(
        owned(&out)
            .find_path("/chosen")
            .unwrap()
            .property("bootargs")
            .unwrap()
            .as_str(),
        Some("two two")
    );
    let no_match =
        "- {op: regex, property: /chosen/bootargs, match: absent, replace: '${MISSING}'}";
    assert_eq!(apply(no_match, &before, None), before);
    let match_it = no_match.replace("absent", "one");
    assert!(
        Transform::parse(Some(&match_it))
            .unwrap()
            .apply(Some(&before), None)
            .is_err()
    );
    let set = "- {op: set, property: /chosen/bootargs, type: string, data: '${CARAPACE}'}";
    assert!(
        Transform::parse(Some(set))
            .unwrap()
            .apply(Some(&before), None)
            .is_err()
    );
}

#[test]
fn captures_and_escaping_do_not_reexpand_values() {
    let before = dtb(Some(string("quiet console=ttyS0 console=ttyS1")));
    let yaml = r"- op: regex
  property: /chosen/bootargs
  match: '(?P<prefix>^|\s)console=ttyS([01])'
  replace: '${prefix}console=hvc${2} $$ $${CARAPACE}'
";
    let out = apply(yaml, &before, None);
    assert_eq!(
        owned(&out)
            .find_path("/chosen")
            .unwrap()
            .property("bootargs")
            .unwrap()
            .as_str(),
        Some("quiet console=hvc0 $ ${CARAPACE} console=hvc1 $ ${CARAPACE}")
    );
}

#[test]
fn invalid_operations_are_rejected_before_execution() {
    for yaml in [
        "- {op: set, property: /x, type: u32, data: 4294967296}",
        "- {op: set, property: /x, type: u32, data: -1}",
        "- {op: set, property: /x, type: bytes, data: 'abc'}",
        "- {op: set, property: /x, type: empty, data: 1}",
        "- {op: set, property: /x, type: string, data: '${1}'}",
        "- {op: regex, property: /x, match: '[', replace: x}",
        "- {op: regex, property: /x, match: x, replace: '${1}'}",
        "- {op: regex, property: /x, match: '(?P<CARAPACE>x)', replace: x}",
        "- {op: regex, property: /x, match: x, replace: '${CARAPACE'}",
        "- {op: regex, property: /x, match: x, replace: x, matches: 1}",
        "- {op: remove, property: /x, type: empty}",
        "- {op: remove, property: /x, property: /y}",
        "- {op: remove, property: /a/../b}",
        "- {op: remove, property: /a//b}",
        "- {op: remove, property: relative}",
    ] {
        assert!(Transform::parse(Some(yaml)).is_err(), "accepted {yaml}");
    }
}

#[test]
fn absent_remove_and_nul_expansion_fail_without_mutating_input() {
    let before = dtb(Some(string("original")));
    let saved = before.clone();
    let yaml = "- {op: set, property: /new, type: u32, data: 1}\n- {op: remove, property: /absent}";
    assert!(
        Transform::parse(Some(yaml))
            .unwrap()
            .apply(Some(&before), None)
            .is_err()
    );
    assert_eq!(before, saved);
    let nul = "- op: set\n  property: /text\n  type: string\n  data: \"a\\0b\"\n";
    assert!(
        Transform::parse(Some(nul))
            .unwrap()
            .apply(Some(&before), None)
            .is_err()
    );
}

#[test]
fn editing_preserves_boot_cpu_and_reservations() {
    let mut before = dtb(Some(string("quiet")));
    before[28..32].copy_from_slice(&7_u32.to_be_bytes());
    // Insert a memory reservation before the existing zero terminator, then
    // adjust structure/strings offsets and total size in this test fixture.
    let reserve = [0x8000_0000_u64.to_be_bytes(), 0x2000_u64.to_be_bytes()].concat();
    before.splice(40..40, reserve);
    for offset in [4, 8, 12] {
        let v = u32::from_be_bytes(before[offset..offset + 4].try_into().unwrap()) + 16;
        before[offset..offset + 4].copy_from_slice(&v.to_be_bytes());
    }
    let out = apply(
        "- {op: set, property: /new, type: u32, data: 42}",
        &before,
        None,
    );
    assert_eq!(&out[28..32], &7_u32.to_be_bytes());
    assert_eq!(owned(&out).reservations(), owned(&before).reservations());
    assert!(
        Transform::parse(Some("[]"))
            .unwrap()
            .apply(Some(b"invalid"), None)
            .is_err()
    );
}
