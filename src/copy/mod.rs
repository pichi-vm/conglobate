// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Validated COPY descriptions; filesystem execution is a separate authority.
mod mode;
pub use mode::{Change, Clause, CopyMode, ModeError, Operator, Permission, Permissions};
