// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]
extern crate self as conglobate;
mod anchor;
// The artifact schema is the library surface (see `src/lib.rs`). Because the
// binary shadows the library crate name via `extern crate self`, it compiles
// the same sources as a private module; items the engine does not itself call
// are the published contract rather than dead code.
#[allow(dead_code, unreachable_pub, unused_imports)]
mod artifact;
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
mod image;
mod input;
mod inventory;
mod isolation;
mod lock;
mod network;
mod output;
mod packet;
mod path;
mod pipeline;
mod plan;
mod process;
mod recipe;
// Portable YAML contract (`BuildOutput` and the recipe/lock types). The engine
// writes `build.yaml` from these types; the host links the same module via the
// library. Most items are unused in the binary.
#[allow(dead_code, unreachable_pub, unused_imports)]
mod recipe_io;
mod request;
mod step;
mod storage;
mod template;
mod transform;
mod value;

#[path = "../tests/lifecycle/src/main.rs"]
mod fixture;
fn main() {
    fixture::main();
}
