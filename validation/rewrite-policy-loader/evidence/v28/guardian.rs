// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{ContainedHelper, HelperCancelFailure, HelperReaping};
use crate::process::{GuardianWatch, Paired, ShutdownTrigger, Supervision};
use std::{io, time::Instant};

/// Registered-phase ownership inside the trusted guardian process. This does
/// not launch that process, monitor setup or own policy/filesystem resources.
#[derive(Debug)]
pub struct RegisteredGuardian<'lease> {
    supervisor: Supervision<Paired>,
    helper: ContainedHelper<'lease>,
}

/// Observation and cancellation have independent outcomes: peer loss or an
/// observation error always requests cancellation, but never proves it finished.
#[derive(Debug)]
pub struct GuardianStop<'lease> {
    observation: io::Result<ShutdownTrigger>,
    cancellation: Result<HelperReaping<'lease>, Box<HelperCancelFailure<'lease>>>,
}
impl<'lease> RegisteredGuardian<'lease> {
    pub fn new(supervisor: Supervision<Paired>, helper: ContainedHelper<'lease>) -> Self {
        Self { supervisor, helper }
    }

    /// Consume both channels on any observation outcome, including timeout or
    /// protocol failure. Cancellation always sends cgroup.kill, even if its
    /// separate absolute deadline has already elapsed. The caller must finish
    /// retained cleanup/reaping and cannot recover a live guardian from here.
    pub fn supervise_until(
        self,
        observation_deadline: Instant,
        cancellation_deadline: Instant,
    ) -> GuardianStop<'lease> {
        let Self { supervisor, helper } = self;
        let observation =
            GuardianWatch::new(&supervisor, helper.supervision()).wait_until(observation_deadline);
        let cancellation = helper.cancel_until(cancellation_deadline);
        // Close upstream after requesting cancellation; this is not a success
        // acknowledgement and must not be interpreted as one by the peer.
        drop(supervisor);
        GuardianStop {
            observation,
            cancellation,
        }
    }
}
impl<'lease> GuardianStop<'lease> {
    pub fn into_parts(
        self,
    ) -> (
        io::Result<ShutdownTrigger>,
        Result<HelperReaping<'lease>, Box<HelperCancelFailure<'lease>>>,
    ) {
        (self.observation, self.cancellation)
    }
}
