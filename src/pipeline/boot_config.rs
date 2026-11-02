// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{artifact, contract::recipe};

#[derive(Debug, Clone)]
pub(crate) struct BootConfig {
    pub(crate) bytes: Vec<u8>,
    pub(crate) root: Option<[u8; 32]>,
}
impl BootConfig {
    pub(super) fn inherited(bytes: Vec<u8>, root: Option<[u8; 32]>) -> anyhow::Result<Self> {
        std::str::from_utf8(&bytes)?.parse::<artifact::BootConfig>()?;
        Ok(Self { bytes, root })
    }
    pub(super) fn overlay(
        base: Option<Self>,
        overlay: &recipe::BootConfig,
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
        let config = overlay.apply(&parsed)?;
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
