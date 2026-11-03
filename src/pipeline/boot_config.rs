// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{boot_config::BootConfig as Native, contract::plan};
use std::io::Read;

#[derive(Debug, Clone)]
pub(crate) struct BootConfig {
    pub(crate) bytes: Vec<u8>,
    pub(crate) root: Option<[u8; 32]>,
}
impl BootConfig {
    pub(super) fn decode(bytes: &[u8]) -> anyhow::Result<Native> {
        if bytes.is_empty() {
            return Ok(Native::default());
        }
        let mut decoder = ruzstd::decoding::StreamingDecoder::new(bytes)?;
        let mut text = String::new();
        decoder.read_to_string(&mut text)?;
        anyhow::ensure!(
            decoder.into_inner().is_empty(),
            "trailing compressed bootconfig bytes"
        );
        Ok(text.parse()?)
    }
    fn encode(config: &Native) -> Vec<u8> {
        if config.values().is_empty() {
            Vec::new()
        } else {
            ruzstd::encoding::compress_to_vec(
                config.as_ref().as_bytes(),
                ruzstd::encoding::CompressionLevel::Fastest,
            )
        }
    }
    pub(super) fn inherited(bytes: Vec<u8>, root: Option<[u8; 32]>) -> anyhow::Result<Self> {
        let parsed = Self::decode(&bytes)?;
        Ok(Self {
            bytes: if parsed.values().is_empty() {
                Vec::new()
            } else {
                bytes
            },
            root,
        })
    }
    pub(super) fn overlay<'a>(
        base: Option<Self>,
        overlay: impl IntoIterator<Item = (&'a plan::Key, Option<&'a plan::Value>)>,
    ) -> anyhow::Result<Self> {
        let parsed = base
            .as_ref()
            .map(|base| Self::decode(&base.bytes))
            .transpose()?
            .unwrap_or_default();
        let mut values = parsed.values().clone();
        for (key, value) in overlay {
            match value {
                None => {
                    values.remove(key.as_ref());
                }
                Some(plan::Value::String(value)) => {
                    values.insert(key.as_ref().to_owned(), vec![value.clone()]);
                }
                Some(plan::Value::List(value)) => {
                    values.insert(key.as_ref().to_owned(), value.clone());
                }
            }
        }
        let config = Native::try_from(values)?;
        Ok(Self {
            bytes: Self::encode(&config),
            root: base.and_then(|base| base.root),
        })
    }
    pub(super) fn bind(&mut self, root: [u8; 32]) -> anyhow::Result<()> {
        let config = Self::decode(&self.bytes)?;
        if config.values().contains_key("corium.root") {
            let mut values = config.values().clone();
            values.insert("corium.root".into(), vec![hex::encode(root)]);
            self.bytes = Self::encode(&Native::try_from(values)?);
        }
        self.root = Some(root);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
    // SPDX-License-Identifier: Apache-2.0

    use super::BootConfig;
    use crate::{boot_config::BootConfig as Native, contract::plan};
    use std::collections::BTreeMap;

    fn overlay(base: Option<BootConfig>, value: serde_json::Value) -> anyhow::Result<Native> {
        let values: BTreeMap<plan::Key, Option<plan::Value>> = serde_json::from_value(value)?;
        let config = BootConfig::overlay(
            base,
            values.iter().map(|(key, value)| (key, value.as_ref())),
        )?;
        BootConfig::decode(&config.bytes)
    }
    #[test]
    fn values_are_literal_and_do_not_expand_shell_or_build_variables() {
        for text in [
            "$${CARAPACE} ${CARAPACE} $$",
            "${word}:$1:$2:$99:${missing}",
            "$(touch /outside)",
            "a; b",
        ] {
            let config = overlay(None, serde_json::json!({"kernel.argument":text})).unwrap();
            assert_eq!(config.values()["kernel.argument"], [text]);
        }
    }
    #[test]
    fn overlays_distinguish_bare_empty_list_and_exact_deletion() {
        let base = BootConfig::inherited(
            ruzstd::encoding::compress_to_vec(
                b"corium = present; corium { root = old; child = keep; }\nkernel.console = ttyS0\n"
                    .as_slice(),
                ruzstd::encoding::CompressionLevel::Fastest,
            ),
            None,
        )
        .unwrap();
        let result = overlay(Some(base), serde_json::json!({"corium":null,"corium.root":"","kernel.console":["ttyS0","hvc0"],"kernel.quiet":[]})).unwrap();
        assert!(!result.values().contains_key("corium"));
        assert_eq!(result.values()["corium.root"], [""]);
        assert_eq!(result.values()["corium.child"], ["keep"]);
        assert_eq!(result.values()["kernel.console"], ["ttyS0", "hvc0"]);
        assert!(result.values()["kernel.quiet"].is_empty());
    }
    #[test]
    fn invalid_keys_and_unrepresentable_values_are_rejected() {
        for (key, value) in [
            ("kernel.bad key", "text"),
            ("kernel.arg", "nul\0"),
            ("kernel.arg", "both '\" quotes; delimiter"),
        ] {
            assert!(overlay(None, serde_json::json!({key:value})).is_err());
        }
    }

    #[test]
    fn compressed_inheritance_preserves_bytes_and_rejects_broken_frames() {
        let text = b"# retain authored spelling\ncorium.root = old\n";
        let bytes = ruzstd::encoding::compress_to_vec(
            text.as_slice(),
            ruzstd::encoding::CompressionLevel::Fastest,
        );
        let mut inherited = BootConfig::inherited(bytes.clone(), None).unwrap();
        assert_eq!(inherited.bytes, bytes);
        inherited.bind([42; 32]).unwrap();
        assert_eq!(
            BootConfig::decode(&inherited.bytes).unwrap().values()["corium.root"],
            [hex::encode([42; 32])]
        );
        assert!(BootConfig::inherited(text.to_vec(), None).is_err());
        assert!(BootConfig::inherited(bytes[..bytes.len() - 1].to_vec(), None).is_err());
        let mut trailing = bytes;
        trailing.push(0);
        assert!(BootConfig::inherited(trailing, None).is_err());
    }

    #[test]
    fn configuration_size_does_not_depend_on_a_pmi_reservation() {
        let text = "x".repeat(64 * 1024);
        let config = overlay(None, serde_json::json!({"kernel.argument": text})).unwrap();
        assert_eq!(config.values()["kernel.argument"], [text]);
    }
}
