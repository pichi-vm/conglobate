// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

/// The Pichi reconstruction parameters carried by an OCI scute descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScuteAnnotations {
    /// Positive COW length rounded up to data blocks.
    pub data_blocks: std::num::NonZeroU64,
    /// Hex-encoded salt, beginning with the previous scute's verity root.
    pub salt: String,
}

impl TryFrom<&oci_spec::image::Descriptor> for ScuteAnnotations {
    type Error = super::ManifestValidationError;

    fn try_from(descriptor: &oci_spec::image::Descriptor) -> Result<Self, Self::Error> {
        let annotations = descriptor.annotations().as_ref();
        let blocks = annotations
            .and_then(|a| a.get("dev.pichi.scute.verity.data-blocks"))
            .filter(|value| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| Self::Error::BadScuteBlocks(descriptor.digest().to_string()))?;
        let salt = annotations
            .and_then(|a| a.get("dev.pichi.scute.verity.salt"))
            .ok_or_else(|| Self::Error::BadSalt(String::new()))?;
        Ok(Self {
            data_blocks: blocks,
            salt: salt.clone(),
        })
    }
}

impl From<ScuteAnnotations> for std::collections::HashMap<String, String> {
    fn from(value: ScuteAnnotations) -> Self {
        [
            (
                "dev.pichi.scute.verity.data-blocks".into(),
                value.data_blocks.to_string(),
            ),
            ("dev.pichi.scute.verity.salt".into(), value.salt),
        ]
        .into()
    }
}
