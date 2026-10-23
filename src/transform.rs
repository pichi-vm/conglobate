// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Property-only DTB edits with immutable input and an execution receipt.
use crate::recipe::{PropertyValue, TransformAction, TransformOperation, TransformSelection};
use devtree::{OwnedNode, OwnedProperty, OwnedTree, Tree};

#[derive(Debug, thiserror::Error)]
pub enum TransformError {
    #[error("invalid DTB: {0}")]
    Dtb(String),
    #[error("transform requires a DTB")]
    MissingDtb,
    #[error("missing DTB property: {0}")]
    MissingProperty(String),
    #[error("DTB property is not one NUL-terminated UTF-8 string: {0}")]
    NotString(String),
    #[error(transparent)]
    Template(#[from] crate::template::TemplateError),
    #[error(transparent)]
    Recipe(#[from] crate::recipe::RecipeError),
}

/// Successful transformation, including the observed default-skip disposition.
#[derive(Debug)]
pub struct TransformedDtb {
    bytes: Option<Vec<u8>>,
    default_skipped: bool,
}
impl TransformedDtb {
    pub fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }
    pub fn default_skipped(&self) -> bool {
        self.default_skipped
    }
}
impl TransformSelection {
    /// Apply selected property edits to an immutable DTB and record default skip.
    /// A net-unchanged tree preserves the original bytes. Only the implicit
    /// default may skip a missing DTB; authored edits still require one.
    ///
    /// # Errors
    /// Rejects malformed DTBs, missing required properties/DTBs, non-string
    /// regex inputs, unavailable template variables and invalid typed values.
    /// A regex with no match succeeds without changing the property.
    pub fn apply(
        &self,
        dtb: Option<&[u8]>,
        carapace: Option<&str>,
    ) -> Result<TransformedDtb, TransformError> {
        let builtin = matches!(self, Self::Builtin);
        let Some(bytes) = dtb else {
            return if builtin {
                Ok(TransformedDtb {
                    bytes: None,
                    default_skipped: true,
                })
            } else {
                Err(TransformError::MissingDtb)
            };
        };
        let tree: Tree<'_> = Tree::parse(bytes).map_err(|e| TransformError::Dtb(e.to_string()))?;
        let original = OwnedTree::from(&tree);
        let mut edited = original.clone();
        let defaults;
        let operations = match self {
            Self::Builtin => {
                defaults = Self::builtin_operations()?;
                &defaults
            }
            Self::Authored(ops) => ops,
        };
        let mut executed = false;
        for op in operations {
            if op
                .exists()
                .is_some_and(|p| PropertyPath::new(p).get(&edited).is_none())
            {
                continue;
            }
            op.apply(&mut edited, carapace)?;
            executed = true;
        }
        let output = if edited == original {
            bytes.to_vec()
        } else {
            let mut output = edited
                .encode()
                .map_err(|e| TransformError::Dtb(e.to_string()))?;
            // The shared owned encoder resets these header fields. Property edits
            // preserve the validated compatibility version and physical boot CPU.
            output[24..32].copy_from_slice(&bytes[24..32]);
            let _: Tree<'_> =
                Tree::parse(&output).map_err(|e| TransformError::Dtb(e.to_string()))?;
            output
        };
        Ok(TransformedDtb {
            bytes: Some(output),
            default_skipped: builtin && !executed,
        })
    }
}

#[derive(Debug)]
struct PropertyPath<'a> {
    path: &'a str,
    node: &'a str,
    name: &'a str,
}
impl<'a> PropertyPath<'a> {
    fn new(path: &'a str) -> Self {
        let (node, name) = path.rsplit_once('/').expect("validated property path");
        Self {
            path,
            node: if node.is_empty() { "/" } else { node },
            name,
        }
    }
    fn missing(&self) -> TransformError {
        TransformError::MissingProperty(self.path.to_owned())
    }
    fn get<'t>(&self, tree: &'t OwnedTree) -> Option<&'t OwnedProperty> {
        tree.find_path(self.node)?.property(self.name)
    }
    fn node_mut<'t>(
        &self,
        tree: &'t mut OwnedTree,
        create: bool,
    ) -> Result<&'t mut OwnedNode, TransformError> {
        let mut node = tree.root_mut();
        for component in self.node.split('/').filter(|s| !s.is_empty()) {
            if node.child(component).is_none() && create {
                node.set_child(OwnedNode::new(component));
            }
            node = node.child_mut(component).ok_or_else(|| self.missing())?;
        }
        Ok(node)
    }
}
impl TransformOperation {
    fn apply(&self, tree: &mut OwnedTree, carapace: Option<&str>) -> Result<(), TransformError> {
        let target = PropertyPath::new(self.property());
        match self.action() {
            TransformAction::Set(value) => {
                let property = value.property(target.name, carapace)?;
                target.node_mut(tree, true)?.set_property(property);
            }
            TransformAction::Remove => {
                target
                    .node_mut(tree, false)?
                    .remove_property(target.name)
                    .ok_or_else(|| target.missing())?;
            }
            TransformAction::Regex {
                pattern,
                replacement,
            } => {
                let raw = target.get(tree).ok_or_else(|| target.missing())?.as_ref();
                let text = raw
                    .strip_suffix(&[0])
                    .filter(|b| !b.contains(&0))
                    .and_then(|b| std::str::from_utf8(b).ok())
                    .ok_or_else(|| TransformError::NotString(target.path.to_owned()))?;
                let mut output = String::new();
                let mut end = 0;
                for captures in pattern.captures_iter(text) {
                    let matched = captures.get(0).expect("regex whole match");
                    output.push_str(&text[end..matched.start()]);
                    output.push_str(&replacement.expand(carapace, Some(&captures))?);
                    end = matched.end();
                }
                output.push_str(&text[end..]);
                target
                    .node_mut(tree, false)?
                    .set_property(OwnedProperty::new(target.name).with_str(&output));
            }
        }
        Ok(())
    }
}
impl PropertyValue {
    fn property(
        &self,
        name: &str,
        carapace: Option<&str>,
    ) -> Result<OwnedProperty, TransformError> {
        let mut p = OwnedProperty::new(name);
        match self {
            Self::String(t) => p.set_str(&t.expand(carapace, None)?),
            Self::Strings(ts) => {
                let strings = ts
                    .iter()
                    .map(|t| t.expand(carapace, None))
                    .collect::<Result<Vec<_>, _>>()?;
                p.set_strs(&strings.iter().map(String::as_str).collect::<Vec<_>>());
            }
            Self::U32(n) => p.set_u32(*n),
            Self::U64(n) => p.set_u64(*n),
            Self::Cells(ns) => p.set_u32s(ns),
            Self::Empty => (),
            Self::Bytes(bytes) => p.set_bytes(bytes.clone()),
        }
        Ok(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{document::YamlDocument, recipe::Recipe};

    struct Fixture;
    impl Fixture {
        fn dtb(property: Option<OwnedProperty>) -> Vec<u8> {
            let mut chosen = OwnedNode::new("chosen");
            if let Some(property) = property {
                chosen.set_property(property);
            }
            let mut bytes = OwnedTree::new(OwnedNode::new("").with_child(chosen))
                .encode()
                .unwrap();
            // Add a nonempty reservation map to exercise preservation during edits.
            let reservation: Vec<u8> = [0x1000u64, 0x2000u64]
                .into_iter()
                .flat_map(u64::to_be_bytes)
                .collect();
            bytes.splice(40..40, reservation);
            for offset in [4, 8, 12] {
                let value = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap());
                bytes[offset..offset + 4].copy_from_slice(&(value + 16).to_be_bytes());
            }
            bytes[28..32].copy_from_slice(&7u32.to_be_bytes());
            bytes
        }
        fn apply(
            ops: &str,
            dtb: Option<&[u8]>,
            root: Option<&str>,
        ) -> Result<TransformedDtb, TransformError> {
            let yaml = format!("carapace:\n  from: example.test/base:1\ntransform: {ops}\n");
            Recipe::try_from(YamlDocument::from(yaml.as_str()))
                .unwrap()
                .transform()
                .apply(dtb, root)
        }
        fn tree(result: &TransformedDtb) -> OwnedTree {
            let tree: Tree<'_> = Tree::parse(result.bytes().unwrap()).unwrap();
            OwnedTree::from(&tree)
        }
    }
    #[test]
    fn default_observes_absence_and_preserves_noops() {
        let default = TransformSelection::Builtin;
        let absent = default.apply(None, None).unwrap();
        assert!(absent.default_skipped());
        assert!(absent.bytes().is_none());
        let dtb = Fixture::dtb(None);
        let skipped = default.apply(Some(&dtb), None).unwrap();
        assert!(skipped.default_skipped());
        assert_eq!(skipped.bytes(), Some(dtb.as_slice()));
        let dtb = Fixture::dtb(Some(OwnedProperty::new("bootargs").with_str("quiet")));
        let unmatched = default.apply(Some(&dtb), None).unwrap();
        assert!(!unmatched.default_skipped());
        assert_eq!(unmatched.bytes(), Some(dtb.as_slice()));
        assert!(Fixture::apply("[]", None, None).is_err());
        let empty = Fixture::apply("[]", Some(&dtb), None).unwrap();
        assert_eq!(empty.bytes(), Some(dtb.as_slice()));
        assert!(!empty.default_skipped());
    }
    #[test]
    fn default_updates_all_matches_and_requires_root_only_for_matches() {
        let dtb = Fixture::dtb(Some(
            OwnedProperty::new("bootargs").with_str("corium=old quiet corium=older"),
        ));
        assert!(TransformSelection::Builtin.apply(Some(&dtb), None).is_err());
        let result = TransformSelection::Builtin
            .apply(Some(&dtb), Some("abcd"))
            .unwrap();
        assert_eq!(
            Fixture::tree(&result)
                .find_path("/chosen")
                .unwrap()
                .property("bootargs")
                .unwrap()
                .as_str(),
            Some("corium=abcd quiet corium=abcd")
        );
        assert_eq!(&result.bytes().unwrap()[24..32], &dtb[24..32]);
        assert_eq!(
            Fixture::tree(&result).reservations(),
            &[devtree::Reservation {
                address: 0x1000,
                size: 0x2000
            }]
        );
        assert!(!result.default_skipped());
        assert!(
            TransformSelection::Builtin
                .apply(Some(&dtb), Some("bad\0root"))
                .is_err()
        );
    }
    #[test]
    fn regex_checks_type_before_matching_and_conditions_before_reading() {
        let op = "\n- op: regex\n  property: /chosen/bootargs\n  match: absent\n  replace: '${CARAPACE}'";
        for bytes in [
            vec![],
            b"unterminated".to_vec(),
            b"a\0b\0".to_vec(),
            vec![255, 0],
        ] {
            let dtb = Fixture::dtb(Some(OwnedProperty::new("bootargs").with_bytes(bytes)));
            assert!(matches!(
                Fixture::apply(op, Some(&dtb), None),
                Err(TransformError::NotString(_))
            ));
        }
        let dtb = Fixture::dtb(None);
        assert!(matches!(
            Fixture::apply(op, Some(&dtb), None),
            Err(TransformError::MissingProperty(_))
        ));
        let conditional = format!("{op}\n  if:\n    exists: /chosen/bootargs");
        assert_eq!(
            Fixture::apply(&conditional, Some(&dtb), None)
                .unwrap()
                .bytes(),
            Some(dtb.as_slice())
        );
        assert!(Fixture::apply(&conditional, None, None).is_err());
        let dtb = Fixture::dtb(Some(OwnedProperty::new("bootargs").with_str("quiet")));
        assert_eq!(
            Fixture::apply(op, Some(&dtb), None).unwrap().bytes(),
            Some(dtb.as_slice())
        );
    }
    #[test]
    fn typed_sets_create_nodes_and_remove_requires_a_property() {
        let dtb = Fixture::dtb(None);
        let ops = "\n- {op: set, property: /new/sub/text, type: string, data: '$$${CARAPACE}'}\n- {op: set, property: /new/sub/list, type: strings, data: [one, two]}\n- {op: set, property: /new/sub/u32, type: u32, data: 4294967295}\n- {op: set, property: /new/sub/u64, type: u64, data: 18446744073709551615}\n- {op: set, property: /new/sub/cells, type: cells, data: [1, 2]}\n- {op: set, property: /new/sub/empty, type: empty}\n- {op: set, property: /new/sub/bytes, type: bytes, data: '00 ff 7A'}\n- {op: remove, property: /new/sub/empty}";
        let result = Fixture::apply(ops, Some(&dtb), Some("abcd")).unwrap();
        let tree = Fixture::tree(&result);
        let node = tree.find_path("/new/sub").unwrap();
        assert_eq!(node.property("text").unwrap().as_ref(), b"$abcd\0");
        assert_eq!(node.property("list").unwrap().as_ref(), b"one\0two\0");
        assert_eq!(
            node.property("u32").unwrap().as_ref(),
            &u32::MAX.to_be_bytes()
        );
        assert_eq!(
            node.property("u64").unwrap().as_ref(),
            &u64::MAX.to_be_bytes()
        );
        assert_eq!(
            node.property("cells").unwrap().as_ref(),
            &[0, 0, 0, 1, 0, 0, 0, 2]
        );
        assert_eq!(node.property("bytes").unwrap().as_ref(), &[0, 255, 122]);
        assert!(node.property("empty").is_none());
        assert!(
            Fixture::apply("\n- {op: remove, property: /missing/p}", Some(&dtb), None).is_err()
        );
        assert!(Fixture::apply("\n- {op: remove, property: /chosen/p}", Some(&dtb), None).is_err());
        assert!(Fixture::apply("[]", Some(b"not a dtb"), None).is_err());
    }
}
