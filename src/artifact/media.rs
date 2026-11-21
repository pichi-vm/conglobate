// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

/// A format identified by one fixed media-type string.
pub trait Media {
    const MEDIA_TYPE: &'static str;
}
