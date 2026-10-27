// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Build-appliance entry: named-console build-port + fixed mounts → engine.

mod console;
mod mounts;
mod request;

use crate::pipeline::Pipeline;
use console::{open_build_console, write_build_input_request};
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
/// v0 wire: guest writes `{"v":1,"op":"build-input"}`, then reads one
/// self-delimiting CBOR response map (`ok:true` + build fields, or `ok:false`
/// + `error`). Guest must not assume the host pushed bytes before the request.
pub(crate) fn run() -> Result<(), ApplianceError> {
    let mut file = open_build_console(Path::new("/sys"), Path::new("/dev"))?;
    write_build_input_request(&mut file)?;
    let request = ApplianceRequest::decode_response_from_port(&mut file)?;
    let build = request.into_build_request(
        mounts::repository(),
        mounts::cache(),
        mounts::output(),
        mounts::id_mappings(),
        mounts::network_namespace().to_path_buf(),
        mounts::resolv_conf().to_path_buf(),
    )?;
    Pipeline::new(build)
        .run()
        .map_err(ApplianceError::Pipeline)?;
    Ok(())
}
