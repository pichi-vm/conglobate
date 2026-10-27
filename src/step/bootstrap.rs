// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{StepStorage, private};
use crate::storage::{BootstrapVolumes, Mounted};
impl private::Storage for BootstrapVolumes<'_, '_, '_, Mounted> {}
impl StepStorage for BootstrapVolumes<'_, '_, '_, Mounted> {}
