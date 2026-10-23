// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Captured trusted formatter inputs. Recipe commands cannot select these files.
//! Runtime staging and privileged execution are separate consuming transitions.
mod runtime;
pub use runtime::{CapturedFormatter, FormatterFile};
mod staged;
pub use staged::{FormatterCleanupFailure, FormatterRuntime, FormatterStageFailure};
