//! Disposable-guest driver for instrumented snapshot creation.
#![forbid(unsafe_code)]
use conglobate::{mounts::Scratch, snapshot::Snapshot};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arg = std::env::args().nth(1).expect("origin or --recover");
    if arg == "--recover-gated" {
        nix::sys::signal::raise(nix::sys::signal::Signal::SIGSTOP)?;
    }
    let scratch = Scratch::new()?;
    if arg == "--recover" || arg == "--recover-gated" {
        drop(scratch);
        return Ok(());
    }
    std::fs::write("/work/lease", scratch.path().as_os_str().as_encoded_bytes())?;
    let origin = std::fs::File::open(arg)?;
    let _snapshot = Snapshot::create(origin, [0; 32], &scratch.path().join("snapshot"))?;
    panic!("requested interruption gate was not reached");
}
