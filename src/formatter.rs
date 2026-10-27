// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Captured trusted formatter inputs. Recipe commands cannot select these files.
//! Runtime staging and privileged execution are separate consuming transitions.
mod runtime;
pub(crate) use runtime::CapturedFormatter;
mod staged;
pub(crate) use staged::FormatterRuntime;
mod profile;
pub(crate) use profile::Ext4Format;

mod input;
pub(crate) use input::FormatterInput;
mod execution;
pub(crate) use execution::{FormatReceipt, FormattedSnapshot};

mod manifest;
pub(crate) mod selection;
