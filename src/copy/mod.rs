// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Validated COPY descriptions; filesystem execution is a separate authority.
mod mode;
pub(crate) use mode::{CopyMode, ModeError};

mod pattern;
pub(crate) use pattern::{PatternError, SourcePattern};

mod source;
pub(crate) use source::{CopySource, CopySources, SelectionError};

mod destination;
pub(crate) use destination::CopyDestination;
pub(crate) use destination::CopyError;
