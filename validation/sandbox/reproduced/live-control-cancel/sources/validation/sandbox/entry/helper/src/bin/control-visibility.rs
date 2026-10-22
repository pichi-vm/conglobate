//! Bounded visibility checks against the disposable build coordinator only.
#![forbid(unsafe_code)]
use std::{error::Error, io::ErrorKind, os::unix::net::UnixStream};
fn main() -> Result<(), Box<dyn Error>> {
    for path in [
        "/work/admission-control/control.sock",
        "/.old/work/admission-control/control.sock",
        "/proc/1/root/work/admission-control/control.sock",
        "/proc/self/root/work/admission-control/control.sock",
    ] {
        match UnixStream::connect(path) {
            Err(error) if error.kind() == ErrorKind::NotFound => (),
            Err(error) => return Err(format!("unexpected control-path error {path}: {error}").into()),
            Ok(_) => return Err(format!("trusted control socket visible: {path}").into()),
        }
    }
    println!("LIVE_RECIPE_CONTROL_SOCKET_PATHS_ABSENT_PASS");
    Ok(())
}
