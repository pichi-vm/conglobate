// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Trusted process entry points for the command executor. They are never recipe
//! entry points and must run in fresh, single-threaded helper processes.
mod helper;
pub use helper::{HelperInvocation, HelperProgram};
