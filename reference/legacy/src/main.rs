// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Standalone image builder and its private execution helpers.

#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use conglobate::{Error, build::Build, execute::Step};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Action,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Build an image from a committed context and locked local images.
    Build(Build),
    #[command(name = "__step", hide = true)]
    Step(Step),
    #[command(name = "__init", hide = true)]
    Init(Step),
}

fn run() -> Result<(), Error> {
    let cli = Cli::parse();
    if !nix::unistd::geteuid().is_root() || !nix::unistd::getuid().is_root() {
        return Err(Error::Invalid("conglobate requires real root".into()));
    }
    match cli.command {
        Action::Step(step) => step.supervisor(),
        Action::Init(step) => step.init(),
        Action::Build(build) => {
            conglobate::cancel::install()?;
            build.run()
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("conglobate: {error}");
            ExitCode::FAILURE
        }
    }
}
