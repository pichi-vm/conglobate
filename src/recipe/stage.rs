// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Create, Directive, Error, Extend, ImagePath, ImageRef, Mount, StageRef};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Stage {
    Create(Create),
    Extend(Extend),
}
impl Stage {
    pub fn source(&self) -> &ImageRef {
        match self {
            Self::Create(stage) => &stage.using,
            Self::Extend(stage) => &stage.extending,
        }
    }
    pub fn directives(&self) -> &[Directive] {
        match self {
            Self::Create(stage) => &stage.derive,
            Self::Extend(stage) => &stage.derive,
        }
    }
    pub fn mounts(&self) -> &BTreeMap<ImagePath, Mount> {
        match self {
            Self::Create(stage) => &stage.mount,
            Self::Extend(stage) => &stage.mount,
        }
    }
    pub fn validate(&self) -> Result<(), Error> {
        if self.directives().is_empty() {
            return Err(Error::from("stage requires a nonempty derive sequence"));
        }
        for directive in self.directives() {
            if let Directive::Exec(exec) = directive {
                exec.validate()?;
            }
        }
        let mut seen = BTreeSet::new();
        for path in self.mounts().keys() {
            let normalized = path.normalized();
            if normalized == "/" || !seen.insert(normalized) {
                return Err(Error::from(
                    "mount paths cannot replace root or name the same location twice",
                ));
            }
        }
        Ok(())
    }
    pub fn dependencies(&self) -> BTreeSet<StageRef> {
        let mut result = BTreeSet::new();
        if let ImageRef::Local(name) = self.source() {
            result.insert(name.clone());
        }
        for directive in self.directives() {
            if let Directive::Copy { copy } = directive
                && let Some(name) = &copy.from
            {
                result.insert(name.clone());
            }
        }
        result
    }
}
