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
#[path = "../../tests/unit/boot_configs.rs"]
mod tests;
