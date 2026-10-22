// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Cooperative cancellation keeps resource guards alive until workers are reaped.

use crate::Error;
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, Ordering},
};

static CANCELLED: OnceLock<Arc<AtomicBool>> = OnceLock::new();

pub fn install() -> Result<(), Error> {
    let flag = CANCELLED.get_or_init(|| Arc::new(AtomicBool::new(false)));
    for signal in [
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGHUP,
    ] {
        signal_hook::flag::register(signal, Arc::clone(flag))?;
    }
    Ok(())
}

pub fn check() -> Result<(), Error> {
    if CANCELLED
        .get()
        .is_some_and(|flag| flag.load(Ordering::Relaxed))
    {
        Err(Error::Invalid("build cancelled".into()))
    } else {
        Ok(())
    }
}
