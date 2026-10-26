// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Build-appliance entry: console CBOR + fixed mounts → shared engine request.

mod console;
mod mounts;
mod request;

use crate::pipeline::Pipeline;
use console::{open_build_console, read_oneshot};
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

/// Read one-shot CBOR from `dev.pichi.build`, build the engine request, run it.
pub(crate) fn run() -> Result<(), ApplianceError> {
    let file = open_build_console(Path::new("/sys"), Path::new("/dev"))?;
    let bytes = read_oneshot(file)?;
    let request = ApplianceRequest::decode(&bytes)?;
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
