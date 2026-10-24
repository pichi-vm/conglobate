// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]
extern crate self as conglobate;
mod anchor;
mod cache;
mod chain;
mod context;
mod copy;
mod document;
mod domain;
mod execution;
mod filesystem;
mod firmware;
mod formatter;
mod image;
mod input;
mod inventory;
mod isolation;
mod lock;
mod network;
mod output;
mod path;
mod pipeline;
mod plan;
mod process;
mod recipe;
mod request;
mod step;
mod storage;
mod template;
mod transform;
mod value;

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
            eprintln!("conglobate: {error:#}");
            return std::process::ExitCode::FAILURE;
        }
    };
    match Pipeline::new(request).run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("conglobate: {error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}
