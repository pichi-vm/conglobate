// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{Ready, Retained, Step, StepStorage, private};
use crate::storage::{BootstrapVolumes, Mounted};
use std::{marker::PhantomData, time::Instant};
impl private::Storage for BootstrapVolumes<'_, '_, '_, Mounted> {}
impl StepStorage for BootstrapVolumes<'_, '_, '_, Mounted> {}
impl<'l> Step<'l, Ready> {
    /// Consume both mounted owners before commands are admitted. Neither can
    /// be extracted until descendant drain, policy removal and finalization.
    pub fn with_bootstrap<'n, 'o, 'w>(
        self,
        volumes: BootstrapVolumes<'n, 'o, 'w, Mounted>,
    ) -> Step<'l, Ready, BootstrapVolumes<'n, 'o, 'w, Mounted>> {
        Step {
            storage: Retained::new(volumes),
            policy: self.policy,
            resources: self.resources,
            _lease: PhantomData,
        }
    }
}
impl Step<'_, Ready, BootstrapVolumes<'_, '_, '_, Mounted>> {
    pub fn send_bootstrap_until(&self, deadline: Instant) -> std::io::Result<()> {
        let volumes = &self.storage.0;
        volumes.tool().prepare_output_mountpoint()?;
        let namespace = self
            .policy
            .policy
            .namespace_for_root(&volumes.tool().root_metadata()?)?;
        self.policy
            .policy
            .namespace_for_root(&volumes.output().root_metadata()?)?;
        self.resources.0.supervision().send_bootstrap_until(
            &volumes.tool().view()?,
            &volumes.output().view()?,
            namespace,
            deadline,
        )
    }
}
