// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]
extern crate self as conglobate;
include!("modules.rs");
#[cfg(test)]
mod tests;
use crate::{
    execution::{HelperInvocation, HelperProgram},
    pipeline::Pipeline,
    request::BuildRequest,
};
fn main() -> std::process::ExitCode {
    let arguments: Vec<_> = std::env::args_os().collect();
    match HelperInvocation::parse(&arguments[1..]) {
        Ok(Some(invocation)) => match HelperProgram::current() {
            Ok(program) => invocation.execute(&program),
            Err(error) => {
                eprintln!("conglobate: {error}");
                return std::process::ExitCode::FAILURE;
            }
        },
        Ok(None) => {}
        Err(error) => {
            eprintln!("conglobate: {error}");
            return std::process::ExitCode::FAILURE;
        }
    }
    let request = match BuildRequest::parse_from(arguments) {
        Ok(request) => request,
        Err(conglobate::request::RequestError::Cli(error)) => {
            let success = error.exit_code() == 0;
            let _ = error.print();
            return if success {
                std::process::ExitCode::SUCCESS
            } else {
                std::process::ExitCode::from(2)
            };
        }
        Err(error) => {
            eprintln!("conglobate: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };
    match Pipeline::new(request).run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("conglobate: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
