// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{artifact, contract::plan};

#[derive(Debug, Clone)]
pub(crate) struct BootConfig {
    pub(crate) bytes: Vec<u8>,
    pub(crate) root: Option<[u8; 32]>,
}
impl BootConfig {
    pub(super) fn inherited(bytes: Vec<u8>, root: Option<[u8; 32]>) -> anyhow::Result<Self> {
        let parsed = std::str::from_utf8(&bytes)?.parse::<artifact::BootConfig>()?;
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
        overlay: impl IntoIterator<
            Item = (
                &'a plan::boot_config::Key,
                Option<&'a plan::boot_config::Value>,
            ),
        >,
    ) -> anyhow::Result<Self> {
        let parsed = base
            .as_ref()
            .map(|base| {
                std::str::from_utf8(&base.bytes)?
                    .parse::<artifact::BootConfig>()
                    .map_err(anyhow::Error::from)
            })
            .transpose()?
            .unwrap_or_default();
        let mut values = parsed.values().clone();
        for (key, value) in overlay {
            match value {
                None => {
                    values.remove(key.as_ref());
                }
                Some(plan::boot_config::Value::String(value)) => {
                    values.insert(key.as_ref().to_owned(), vec![value.clone()]);
                }
                Some(plan::boot_config::Value::List(value)) => {
                    values.insert(key.as_ref().to_owned(), value.clone());
                }
            }
        }
        let config = artifact::BootConfig::try_from(values)?;
        Ok(Self {
            bytes: config.as_ref().as_bytes().to_vec(),
            root: base.and_then(|base| base.root),
        })
    }
    pub(super) fn bind(&mut self, root: [u8; 32]) -> anyhow::Result<()> {
        let config: artifact::BootConfig = std::str::from_utf8(&self.bytes)?.parse()?;
        if config.values().contains_key("corium.root") {
            let mut values = config.values().clone();
            values.insert("corium.root".into(), vec![hex::encode(root)]);
            self.bytes = artifact::BootConfig::try_from(values)?
                .as_ref()
                .as_bytes()
                .to_vec();
        }
        self.root = Some(root);
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/unit/boot_configs.rs"]
mod tests;
