// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Terminal request/result exchange around the build engine.

mod console;
mod request;

use crate::pipeline::Pipeline;
use crate::request::{BuildRequest, RuntimeArguments};
use console::{open_build_console, write_build_input_request, write_build_result};
use std::path::Path;

/// Open the well-known protocol console, receive build input and run the engine.
///
/// Request input with `start`, then report the result with `finish` and
/// wait for its acknowledgement before returning to the caller.
pub(crate) fn run(runtime: RuntimeArguments) -> anyhow::Result<()> {
    let mut file = open_build_console(Path::new("/sys"), Path::new("/dev"))?;
    write_build_input_request(&mut file)?;
    let result = (|| -> anyhow::Result<()> {
        let input = request::decode_response_from_port(&mut file)?;
        let build = BuildRequest::from_terminal_input(input, runtime)?;
        Pipeline::new(build).run()
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
