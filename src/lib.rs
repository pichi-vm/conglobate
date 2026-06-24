// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! conglobate's library half: the `pichi.build` recipe I/O types shared
//! with the host — parse + validate for `carapace.yaml`, `pmi.yaml`, and
//! `refs.lock` (BUILD.md §2.1, §2.2, §2.4).
//!
//! Shared by the host (`pichi build` / `pichi update`) and the in-guest
//! build driver (`conglobate`). The types are the on-disk YAML schema; a
//! `parse` constructor on each runs `serde_yaml` then a structural
//! `validate` pass. MVP scope — the `config.yaml` → `requirements.yaml`
//! filter is deferred.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Recipe parse / validation failures.
#[derive(Debug, Error)]
pub enum RecipeError {
    #[error("parse: {0}")]
    Parse(#[from] serde_yaml::Error),

    #[error("invalid recipe: {0}")]
    Invalid(String),
}

/// `pichi.build/carapace.yaml` — derive a read-only carapace from one base
/// registry reference (BUILD.md §2.1).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CarapaceRecipe {
    /// Exactly one registry reference to derive from (v1: no
    /// `raw:`/`tarball:`/`oci:` — run `pichi import` first).
    pub from: String,

    /// Ordered directives, each producing one retained scute. Optional.
    #[serde(default)]
    pub derive: Vec<Directive>,
}

/// `pichi.build/pmi.yaml` — produce a single `.pmi` (BUILD.md §2.2).
/// Nothing but the file named by `into` is retained; steps are not
/// materialized as scutes.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PmiRecipe {
    /// Omit to build against the carapace produced by `carapace.yaml`
    /// (the common case). Required when there is no `carapace.yaml`, or
    /// to build against a different carapace.
    #[serde(default)]
    pub from: Option<String>,

    /// Ordered `run:`/`copy:` sequence run in a working filesystem; all
    /// intermediate state is discarded.
    #[serde(default)]
    pub derive: Vec<Directive>,

    /// Path where the `derive` sequence writes the finished `.pmi`.
    /// Required; no default.
    pub into: String,
}

/// One ordered build directive: run a shell command, or copy files from
/// the build context into the guest filesystem.
///
/// On the wire each directive is a single-key mapping (`{run: …}` or
/// `{copy: …}`) — serde_yaml reserves the externally-tagged enum form for
/// YAML `!tags`, so [`Deserialize`] is hand-written over a `{run?, copy?}`
/// shape that enforces exactly one key.
#[derive(Debug, Clone)]
pub enum Directive {
    /// Execute a shell command inside the build VM (working dir `/`);
    /// tools come from the parent scute.
    Run(String),

    /// Copy from the build context into the guest filesystem, optionally
    /// setting ownership/mode in the same scute.
    Copy(CopyDirective),
}

impl<'de> Deserialize<'de> for Directive {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            #[serde(default)]
            run: Option<String>,
            #[serde(default)]
            copy: Option<CopyDirective>,
        }
        let raw = Raw::deserialize(deserializer)?;
        match (raw.run, raw.copy) {
            (Some(cmd), None) => Ok(Directive::Run(cmd)),
            (None, Some(c)) => Ok(Directive::Copy(c)),
            (None, None) => Err(serde::de::Error::custom(
                "directive must be a `run:` or `copy:` mapping",
            )),
            (Some(_), Some(_)) => Err(serde::de::Error::custom(
                "directive has both `run` and `copy`; use one per list item",
            )),
        }
    }
}

/// `copy:` directive payload (BUILD.md §2.1).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CopyDirective {
    /// A single source path or a list of paths (relative to the build
    /// context). When a list, `into` must be a directory.
    pub from: FromSpec,

    /// Destination path in the guest filesystem.
    pub into: String,

    /// Owner to set (a name resolved against the parent scute's
    /// `/etc/passwd`, or a numeric id as a quoted string). Optional.
    #[serde(default)]
    pub owner: Option<String>,

    /// Group to set (name or quoted numeric id). Optional.
    #[serde(default)]
    pub group: Option<String>,

    /// Quoted octal mode (e.g. `"0755"`). A build error when `from` is a
    /// list (see [`CarapaceRecipe::validate`]). Optional.
    #[serde(default)]
    pub mode: Option<String>,
}

/// A `copy:` source: one path or many.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum FromSpec {
    One(String),
    Many(Vec<String>),
}

impl FromSpec {
    /// True when this is a list of paths (`into` must be a directory).
    pub fn is_list(&self) -> bool {
        matches!(self, FromSpec::Many(_))
    }
}

/// `pichi.build/refs.lock` — machine-written ref → (manifest, carapace)
/// pins (BUILD.md §2.4). The two hashes are independent commitments to
/// the same content: `manifest` is a flat SHA-256 over the OCI manifest
/// bytes; `carapace` is the dm-verity root over scute blocks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RefsLock {
    pub entries: BTreeMap<String, LockEntry>,
}

/// One `refs.lock` entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockEntry {
    /// SHA-256 over the OCI manifest bytes (`sha256:…`).
    pub manifest: String,

    /// dm-verity Merkle root over the carapace's scute blocks (`sha256:…`).
    pub carapace: String,
}

/// The build VM's output manifest (`output/build.yaml`), written by
/// conglobate and read by `pichi build` to package the artifact. Filenames
/// are relative to the output sink directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildOutput {
    /// Scutes in chain order (base first, top last).
    pub scutes: Vec<ScuteOut>,

    /// The sealed PMI filename, if `pmi.yaml` ran. Absent for a
    /// carapace-only build.
    #[serde(default)]
    pub pmi: Option<String>,
}

/// One emitted scute in [`BuildOutput`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScuteOut {
    /// COW blob filename (relative to the output dir).
    pub cow: String,

    /// dm-verity blob filename (relative to the output dir).
    pub verity: String,

    /// Full dm-verity salt, hex (the scute's salt-chain prefix + suffix).
    pub salt: String,
}

impl BuildOutput {
    /// Parse `output/build.yaml`.
    pub fn parse(yaml: &str) -> Result<Self, RecipeError> {
        let out: Self = serde_yaml::from_str(yaml)?;
        Ok(out)
    }

    /// Serialize to YAML (what conglobate writes).
    pub fn to_yaml(&self) -> Result<String, RecipeError> {
        Ok(serde_yaml::to_string(self)?)
    }
}

impl CarapaceRecipe {
    /// Parse and validate a `carapace.yaml` document.
    pub fn parse(yaml: &str) -> Result<Self, RecipeError> {
        let recipe: Self = serde_yaml::from_str(yaml)?;
        recipe.validate()?;
        Ok(recipe)
    }

    /// Structural checks beyond serde's shape enforcement.
    fn validate(&self) -> Result<(), RecipeError> {
        if self.from.trim().is_empty() {
            return Err(RecipeError::Invalid(
                "carapace.yaml: `from` is empty".into(),
            ));
        }
        validate_directives(&self.derive)
    }
}

impl PmiRecipe {
    /// Parse and validate a `pmi.yaml` document.
    pub fn parse(yaml: &str) -> Result<Self, RecipeError> {
        let recipe: Self = serde_yaml::from_str(yaml)?;
        recipe.validate()?;
        Ok(recipe)
    }

    fn validate(&self) -> Result<(), RecipeError> {
        if self.into.trim().is_empty() {
            return Err(RecipeError::Invalid("pmi.yaml: `into` is empty".into()));
        }
        if let Some(from) = &self.from
            && from.trim().is_empty()
        {
            return Err(RecipeError::Invalid(
                "pmi.yaml: `from` is present but empty (omit it to build against carapace.yaml)"
                    .into(),
            ));
        }
        validate_directives(&self.derive)
    }
}

impl RefsLock {
    /// Parse a `refs.lock` document. An empty document is a valid empty
    /// lock (a project with no carapace references).
    pub fn parse(yaml: &str) -> Result<Self, RecipeError> {
        if yaml.trim().is_empty() {
            return Ok(Self::default());
        }
        let lock: Self = serde_yaml::from_str(yaml)?;
        for (reference, entry) in &lock.entries {
            entry.validate(reference)?;
        }
        Ok(lock)
    }

    /// Look up the pins for a reference exactly as written in a recipe.
    pub fn get(&self, reference: &str) -> Option<&LockEntry> {
        self.entries.get(reference)
    }

    /// Serialize to YAML (the body of `refs.lock`, sans the generated-file
    /// header `pichi update` prepends). Keys are emitted in sorted order
    /// (`BTreeMap`) for a stable, diff-friendly file.
    pub fn to_yaml(&self) -> Result<String, RecipeError> {
        Ok(serde_yaml::to_string(self)?)
    }
}

impl LockEntry {
    fn validate(&self, reference: &str) -> Result<(), RecipeError> {
        for (field, value) in [("manifest", &self.manifest), ("carapace", &self.carapace)] {
            if !value.starts_with("sha256:") || value.len() <= "sha256:".len() {
                return Err(RecipeError::Invalid(format!(
                    "refs.lock: {reference}: `{field}` is not a sha256 digest: {value:?}"
                )));
            }
        }
        Ok(())
    }
}

/// `mode` is a build error when `from` is a list (BUILD.md §2.1). The
/// single-path-resolving-to-a-directory case is a guest-side runtime
/// check (it needs the filesystem) and is not enforced here.
fn validate_directives(derive: &[Directive]) -> Result<(), RecipeError> {
    for d in derive {
        if let Directive::Copy(c) = d
            && c.mode.is_some()
            && c.from.is_list()
        {
            return Err(RecipeError::Invalid(format!(
                "copy into {:?}: `mode` is not allowed when `from` is a list of paths",
                c.into
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_carapace_recipe() {
        let yaml = r#"
from: registry.example.com/base/fedora:43
derive:
  - run: dnf install -y python3 torch
  - copy:
      from: ./app
      into: /opt/app
      owner: appuser
      group: appuser
      mode: "0755"
"#;
        let r = CarapaceRecipe::parse(yaml).unwrap();
        assert_eq!(r.from, "registry.example.com/base/fedora:43");
        assert_eq!(r.derive.len(), 2);
        match &r.derive[0] {
            Directive::Run(cmd) => assert_eq!(cmd, "dnf install -y python3 torch"),
            other @ Directive::Copy(_) => panic!("expected run, got {other:?}"),
        }
        match &r.derive[1] {
            Directive::Copy(c) => {
                assert!(matches!(c.from, FromSpec::One(_)));
                assert_eq!(c.into, "/opt/app");
                assert_eq!(c.owner.as_deref(), Some("appuser"));
                assert_eq!(c.mode.as_deref(), Some("0755"));
            }
            other @ Directive::Run(_) => panic!("expected copy, got {other:?}"),
        }
    }

    #[test]
    fn carapace_recipe_derive_is_optional() {
        let r = CarapaceRecipe::parse("from: reg/base:1").unwrap();
        assert!(r.derive.is_empty());
    }

    #[test]
    fn copy_accepts_a_list_of_sources() {
        let yaml = r"
from: reg/base:1
derive:
  - copy:
      from: [pichi.build/config.yaml, pichi.build/refs.lock]
      into: /usr/lib/corium/
";
        let r = CarapaceRecipe::parse(yaml).unwrap();
        match &r.derive[0] {
            Directive::Copy(c) => match &c.from {
                FromSpec::Many(v) => assert_eq!(v.len(), 2),
                FromSpec::One(_) => panic!("expected list"),
            },
            other @ Directive::Run(_) => panic!("expected copy, got {other:?}"),
        }
    }

    #[test]
    fn mode_with_list_source_is_rejected() {
        let yaml = r#"
from: reg/base:1
derive:
  - copy:
      from: [a, b]
      into: /dst/
      mode: "0644"
"#;
        let err = CarapaceRecipe::parse(yaml).unwrap_err();
        assert!(matches!(err, RecipeError::Invalid(_)), "{err:?}");
    }

    #[test]
    fn empty_from_is_rejected() {
        let err = CarapaceRecipe::parse("from: \"\"").unwrap_err();
        assert!(matches!(err, RecipeError::Invalid(_)), "{err:?}");
    }

    #[test]
    fn unknown_field_is_rejected() {
        let err = CarapaceRecipe::parse("from: reg/base:1\nbogus: 1").unwrap_err();
        assert!(matches!(err, RecipeError::Parse(_)), "{err:?}");
    }

    #[test]
    fn parses_full_pmi_recipe() {
        let yaml = r"
from: registry.example.com/base/kernel-builder:latest
derive:
  - run: dnf install -y kernel corium dracut
  - copy:
      from: [pichi.build/config.yaml, pichi.build/refs.lock]
      into: /usr/lib/corium/
  - run: dracut --add corium /tmp/initramfs.img
  - run: arma build --kernel /boot/vmlinuz-* --initramfs /tmp/initramfs.img -o /tmp/boot.pmi
into: /tmp/boot.pmi
";
        let r = PmiRecipe::parse(yaml).unwrap();
        assert_eq!(
            r.from.as_deref(),
            Some("registry.example.com/base/kernel-builder:latest")
        );
        assert_eq!(r.into, "/tmp/boot.pmi");
        assert_eq!(r.derive.len(), 4);
    }

    #[test]
    fn pmi_recipe_from_is_optional() {
        let r = PmiRecipe::parse("into: /tmp/x.pmi").unwrap();
        assert!(r.from.is_none());
        assert!(r.derive.is_empty());
    }

    #[test]
    fn pmi_recipe_requires_into() {
        let err = PmiRecipe::parse("from: reg/b:1").unwrap_err();
        assert!(matches!(err, RecipeError::Parse(_)), "{err:?}");
    }

    #[test]
    fn parses_refs_lock() {
        let yaml = r"
registry.example.com/models/llama:7b:
  manifest: sha256:abcdef0000000000000000000000000000000000000000000000000000000000
  carapace: sha256:def0000000000000000000000000000000000000000000000000000000000000
";
        let lock = RefsLock::parse(yaml).unwrap();
        let e = lock.get("registry.example.com/models/llama:7b").unwrap();
        assert!(e.manifest.starts_with("sha256:"));
        assert!(e.carapace.starts_with("sha256:"));
        assert!(lock.get("missing").is_none());
    }

    #[test]
    fn empty_refs_lock_is_valid() {
        assert!(RefsLock::parse("").unwrap().entries.is_empty());
        assert!(RefsLock::parse("   \n").unwrap().entries.is_empty());
    }

    #[test]
    fn refs_lock_rejects_non_sha256_digest() {
        let yaml = "reg/x:1:\n  manifest: deadbeef\n  carapace: sha256:aa\n";
        let err = RefsLock::parse(yaml).unwrap_err();
        assert!(matches!(err, RecipeError::Invalid(_)), "{err:?}");
    }
}
