// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]
mod formatter_failures;
use conglobate::execution::{HelperInvocation, HelperProgram};
use std::{ffi::OsStr, path::Path};

#[derive(Debug)]
struct Lifecycle;
impl Lifecycle {
    fn run() -> Result<(), Box<dyn std::error::Error>> {
        if !Path::new("/conglobate-disposable-validation-vm").is_file() {
            return Err("disposable guest required".into());
        }
        let mut args = std::env::args_os();
        if args.next().as_deref() == Some(OsStr::new("/usr/bin/formatter-stall")) {
            formatter_failures::FormatterFailures::stall();
        }
        let args: Vec<_> = args.collect();
        if let Some(invocation) = HelperInvocation::parse(&args)? {
            invocation.execute(&HelperProgram::current()?);
        }
        match args.as_slice() {
            [mode] if mode == "formatter-failures" => formatter_failures::FormatterFailures::run(),
            _ => Err("expected formatter-failures or trusted helper invocation".into()),
        }
    }
}
fn main() {
    if let Err(error) = Lifecycle::run() {
        eprintln!("lifecycle fixture: {error}");
        std::process::exit(1);
    }
}
