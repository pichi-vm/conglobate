// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]
extern crate conglobate as contract;
extern crate self as conglobate;
mod anchor;
mod appliance;
mod cache;
mod chain;
mod context;
mod copy;
mod deadline;
mod document;
mod domain;
mod execution;
mod filesystem;
mod firmware;
mod formatter;
mod graph;
mod image;
mod input;
mod isolation;
mod lock;
mod network;
mod output;
mod packet;
mod path;
mod pipeline;
mod plan;
mod process;
mod request;
mod step;
mod storage;
mod template;
mod transform;
use contract::{artifact, value};

#[cfg(test)]
mod tests;
use crate::{
    execution::{HelperInvocation, HelperProgram},
    pipeline::Pipeline,
};
fn main() -> std::process::ExitCode {
    let arguments: Vec<_> = std::env::args_os().collect();
    match HelperInvocation::parse(&arguments[1..]) {
        Ok(Some(invocation)) => match HelperProgram::current() {
            Ok(program) => invocation.execute(&program),
            Err(error) => {
                eprintln!("conglobate: {error:#}");
                return std::process::ExitCode::FAILURE;
            }
        },
        Ok(None) => {}
        Err(error) => {
            eprintln!("conglobate: {error:#}");
            return std::process::ExitCode::FAILURE;
        }
    }
    let command = match request::parse_cli(arguments) {
        Ok(command) => command,
        Err(request::RequestError::Cli(error)) => {
            let success = error.exit_code() == 0;
            let _ = error.print();
            return if success {
                std::process::ExitCode::SUCCESS
            } else {
                std::process::ExitCode::from(2)
            };
        }
        Err(error) => {
            eprintln!("conglobate: {error:#}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let result = match command {
        request::CliCommand::Build(request) => Pipeline::new(*request).run(),
        request::CliCommand::Appliance => crate::appliance::run().map_err(Into::into),
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("conglobate: {error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}
