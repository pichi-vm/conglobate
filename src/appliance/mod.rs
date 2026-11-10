// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Build-appliance entry: named-console build-port + fixed mounts → engine.

mod console;
mod mounts;
mod request;

use crate::pipeline::Pipeline;
use console::{open_build_console, write_build_input_request, write_build_result};
use request::ApplianceRequest;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ApplianceError {
    #[error(transparent)]
    Console(#[from] console::ConsoleError),
    #[error(transparent)]
    Request(#[from] request::ApplianceRequestError),
    #[error(transparent)]
    Pipeline(anyhow::Error),
}

/// Open `dev.pichi.build`, request build-input, decode the host response, run.
///
/// Request input with `start`, then report the result with `finish` and
/// wait for its acknowledgement before the appliance shuts down.
pub(crate) fn run() -> Result<(), ApplianceError> {
    let mut file = open_build_console(Path::new("/sys"), Path::new("/dev"))?;
    write_build_input_request(&mut file)?;
    let result = (|| -> Result<(), ApplianceError> {
        let request = ApplianceRequest::decode_response_from_port(&mut file)?;
        let build = request.into_build_request(
            mounts::repository(),
            mounts::cache(),
            mounts::output(),
            mounts::id_mappings(),
        )?;
        Pipeline::new(build)
            .run_with_resolver(|| {
                let status = std::process::Command::new("/usr/bin/systemctl")
                    .args(["start", "bootstrap-time.service"])
                    .status()?;
                if !status.success() {
                    return Err(std::io::Error::other(format!(
                        "network bootstrap failed: {status}"
                    )));
                }
                crate::network::ResolverInput::capture(Path::new(
                    "/run/systemd/resolve/resolv.conf",
                ))
            })
            .map_err(ApplianceError::Pipeline)
    })();
    let reported = write_build_result(
        &mut file,
        result
            .as_ref()
            .map(|_| ())
            .map_err(|error| format!("{error:#}")),
    );
    // Preserve the build failure if the host also disconnected while reporting.
    result?;
    reported?;
    Ok(())
}
