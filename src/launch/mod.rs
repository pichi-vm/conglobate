// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Typed requests and responses for the bidirectional appliance console.
//! Commands version their own request and response payloads. Transport and
//! filesystem resolution are outside this portable contract.

mod build;
mod command;
mod input;
mod request;
mod requests;
mod response;

pub use build::Build;
pub use command::Command;
pub use input::Input;
pub use request::Request;
pub use requests::Requests;
pub use response::Response;

/// Named bidirectional console used by the build appliance.
pub const PORT_NAME: &str = "dev.pichi.build";

#[cfg(test)]
mod tests;
