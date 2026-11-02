// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]
extern crate conglobate as contract;
extern crate self as conglobate;
mod anchor;
mod disk;
use contract::artifact;
mod cache;
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
#[allow(dead_code)]
mod inspect;
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
mod value;

#[path = "../tests/lifecycle/src/main.rs"]
mod fixture;
fn main() {
    fixture::main();
}
