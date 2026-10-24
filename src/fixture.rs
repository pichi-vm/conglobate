// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]
extern crate self as conglobate;
include!("modules.rs");
#[path = "../tests/lifecycle/src/main.rs"]
mod fixture;
fn main() { fixture::main(); }
