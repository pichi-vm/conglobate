// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::StepStorage;
use crate::{
    context::CapturedContext,
    copy::CopyError,
    isolation::{Mapped, UserNamespace},
    network::CommandNetwork,
    process::{Joined, Supervision},
    recipe::Copy,
    storage::{BootstrapVolumes, Mounted, Unmounted, Volume},
};
use std::{fmt::Debug, io, path::Path, time::Instant};

/// Assigned storage for the common preparation, command and finalization path.
/// Only successful unmount and snapshot validation return usable stage storage.
pub(crate) trait CommandStorage: StepStorage + Sized {
    type Unmounted: Debug;
    /// Validate every snapshot in the unmounted layout before accepting it.
    #[doc(hidden)]
    fn validate(storage: &Self::Unmounted) -> io::Result<()>;
    #[doc(hidden)]
    fn mount(volume: Self::Unmounted, parent: &Path, deadline: Instant) -> anyhow::Result<Self>;
    #[doc(hidden)]
    fn validate_command_root(&self) -> io::Result<()>;
    #[doc(hidden)]
    fn validate_ownership(
        &self,
        uid: crate::isolation::IdRange,
        gid: crate::isolation::IdRange,
        deadline: Instant,
    ) -> io::Result<()>;
    #[doc(hidden)]
    fn unmount_until(self, deadline: Instant) -> io::Result<Self::Unmounted>;
    fn send(
        &self,
        namespace: &UserNamespace<Mapped>,
        channel: &Supervision<Joined>,
        network: Option<&CommandNetwork>,
        deadline: Instant,
    ) -> io::Result<()>;
    fn copy(&mut self, context: &CapturedContext, directive: &Copy) -> Result<(), CopyError>;
    fn copy_artifact(
        &mut self,
        directive: &Copy,
        source: &crate::firmware::FirmwareFile<'_>,
    ) -> Result<(), CopyError>;
}
impl<'n, 'o, 'w> CommandStorage for Volume<'n, 'o, 'w, Mounted> {
    type Unmounted = Volume<'n, 'o, 'w, Unmounted>;
    fn validate(storage: &Self::Unmounted) -> io::Result<()> {
        storage.validate_snapshot()
    }
    fn mount(volume: Self::Unmounted, parent: &Path, deadline: Instant) -> anyhow::Result<Self> {
        crate::step::teardown::Deadline::check(deadline)?;
        Ok(volume.mount_ext4(parent)?)
    }
    fn validate_command_root(&self) -> io::Result<()> {
        self.validate_command_root()
    }
    fn validate_ownership(
        &self,
        uid: crate::isolation::IdRange,
        gid: crate::isolation::IdRange,
        deadline: Instant,
    ) -> io::Result<()> {
        self.validate_ownership(uid, gid, deadline)
    }
    fn unmount_until(self, deadline: Instant) -> io::Result<Self::Unmounted> {
        if let Err(error) = crate::step::teardown::Deadline::check(deadline) {
            std::mem::forget(self);
            return Err(error);
        }
        let volume = self.unmount()?;
        crate::step::teardown::Deadline::check(deadline)?;
        Ok(volume)
    }
    fn send(
        &self,
        namespace: &UserNamespace<Mapped>,
        channel: &Supervision<Joined>,
        network: Option<&CommandNetwork>,
        deadline: Instant,
    ) -> io::Result<()> {
        let view = self.view()?;
        match network {
            Some(network) => channel.send_network_volume_until(
                &view,
                None,
                &self.view()?,
                namespace,
                network,
                deadline,
            ),
            None => channel.send_volume_until(&view, namespace, deadline),
        }
    }
    fn copy(&mut self, context: &CapturedContext, directive: &Copy) -> Result<(), CopyError> {
        self.copy(context, directive)
    }
    fn copy_artifact(
        &mut self,
        directive: &Copy,
        source: &crate::firmware::FirmwareFile<'_>,
    ) -> Result<(), CopyError> {
        self.copy_artifact(None, directive, source)
    }
}
impl<'n, 'o, 'w> CommandStorage for BootstrapVolumes<'n, 'o, 'w, Mounted> {
    type Unmounted = BootstrapVolumes<'n, 'o, 'w, Unmounted>;
    fn validate(storage: &Self::Unmounted) -> io::Result<()> {
        storage.validate_snapshots()
    }
    fn mount(volume: Self::Unmounted, parent: &Path, deadline: Instant) -> anyhow::Result<Self> {
        volume.mount_ext4_until(parent, deadline)
    }
    fn validate_command_root(&self) -> io::Result<()> {
        self.tool().validate_command_root()
    }
    fn validate_ownership(
        &self,
        uid: crate::isolation::IdRange,
        gid: crate::isolation::IdRange,
        deadline: Instant,
    ) -> io::Result<()> {
        self.tool().validate_ownership(uid, gid, deadline)?;
        self.output().validate_ownership(uid, gid, deadline)
    }
    fn unmount_until(self, deadline: Instant) -> io::Result<Self::Unmounted> {
        self.unmount_until(deadline)
    }
    fn send(
        &self,
        namespace: &UserNamespace<Mapped>,
        channel: &Supervision<Joined>,
        network: Option<&CommandNetwork>,
        deadline: Instant,
    ) -> io::Result<()> {
        self.tool().prepare_output_mountpoint()?;
        let root = self.tool().view()?;
        let output = self.output().view()?;
        match network {
            Some(network) => channel.send_network_volume_until(
                &root,
                Some(&output),
                &self.tool().view()?,
                namespace,
                network,
                deadline,
            ),
            None => channel.send_bootstrap_until(&root, &output, namespace, deadline),
        }
    }
    fn copy(&mut self, context: &CapturedContext, directive: &Copy) -> Result<(), CopyError> {
        self.copy(context, directive)
    }
    fn copy_artifact(
        &mut self,
        directive: &Copy,
        source: &crate::firmware::FirmwareFile<'_>,
    ) -> Result<(), CopyError> {
        self.copy_artifact(directive, source)
    }
}
