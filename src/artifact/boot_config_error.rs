// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

/// Invalid native Linux boot configuration or an unrepresentable overlay.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid boot configuration: {0}")]
pub struct BootConfigError(pub(crate) String);
