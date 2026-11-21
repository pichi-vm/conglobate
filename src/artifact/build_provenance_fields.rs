// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{AsString, BuildProvenance, Digest};
use serde::Deserialize;

/// Wire fields before checking whether the provenance group is complete.
#[derive(Deserialize)]
pub(super) struct BuildProvenanceFields {
    #[serde(rename = "dev.pichi.build.commit")]
    commit: Option<Digest<[u8; 20]>>,
    #[serde(rename = "dev.pichi.build.context")]
    context: Option<String>,
    #[serde(rename = "dev.pichi.build.plan")]
    plan: Option<String>,
    #[serde(rename = "dev.pichi.build.lock")]
    lock: Option<String>,
    #[serde(rename = "org.opencontainers.image.source")]
    source: Option<String>,
    #[serde(rename = "dev.pichi.build.bootstrapped")]
    bootstrapped: Option<AsString<bool>>,
}

impl TryFrom<BuildProvenanceFields> for Option<BuildProvenance> {
    type Error = &'static str;

    fn try_from(fields: BuildProvenanceFields) -> Result<Self, Self::Error> {
        match (fields.commit, fields.context, fields.plan) {
            (None, None, None)
                if fields.lock.is_none()
                    && fields.source.is_none()
                    && fields.bootstrapped.is_none() =>
            {
                Ok(None)
            }
            (Some(commit), Some(context), Some(plan)) => Ok(Some(BuildProvenance {
                commit,
                context,
                plan,
                lock: fields.lock,
                source: fields.source,
                bootstrapped: fields.bootstrapped.unwrap_or_default(),
            })),
            _ => Err("build provenance requires commit, context, and plan"),
        }
    }
}
