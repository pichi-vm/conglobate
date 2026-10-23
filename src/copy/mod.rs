// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Validated COPY descriptions; filesystem execution is a separate authority.
mod mode;
pub use mode::{CopyMode, ModeError};

mod pattern;
pub use pattern::{PatternError, SourcePattern};

mod source;
pub use source::{CopySource, CopySources, SelectionError};

mod destination;
pub(crate) use destination::CopyDestination;
pub use destination::CopyError;
