// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{Ready, Retained, Step, StepStorage, private};
use crate::storage::{BootstrapVolumes, Mounted};
use std::marker::PhantomData;
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
