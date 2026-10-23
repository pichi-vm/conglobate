// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use crate::{
    anchor::{FilesystemPin, FilesystemPinFailure, FilesystemPinTarget, RecoveryDirectory},
    step::StepStorage,
    storage::{
        BootstrapMountFailure, BootstrapUnmountFailure, BootstrapVolumes, FailureDiagnostics,
        Mounted, Unmounted, Volume, VolumeUnmountFailure,
    },
};
use std::{fmt::Debug, io, path::Path, time::Instant};

/// Sealed mounted storage layouts sharing the command preparation lifecycle.
/// Cleanup preserves the concrete unmounted layout and its owning retry error.
pub trait StorageLayout: StepStorage + Sized {
    type Unmounted: Debug;
}
pub trait PreparationStorage: StorageLayout {
    type MountFailure: Debug;
    #[doc(hidden)]
    fn mount(
        volume: Self::Unmounted,
        parent: &Path,
        deadline: Instant,
    ) -> Result<Self, (FailureDiagnostics, Self::MountFailure)>;
    #[doc(hidden)]
    fn rollback_mount(
        failure: Self::MountFailure,
        deadline: Instant,
    ) -> Result<Self::Unmounted, Self::UnmountFailure>;
    #[doc(hidden)]
    fn pin(&self, recovery: &RecoveryDirectory) -> Result<Vec<FilesystemPin>, StoragePinFailure>;
    #[doc(hidden)]
    fn validate_command_root(&self) -> io::Result<()>;
    type UnmountFailure: Debug;
    #[doc(hidden)]
    fn unmount_until(self, deadline: Instant) -> Result<Self::Unmounted, Self::UnmountFailure>;
    #[doc(hidden)]
    fn retry_unmount(
        failure: Self::UnmountFailure,
    ) -> Result<Self::Unmounted, Self::UnmountFailure>;
    #[doc(hidden)]
    fn unmount_diagnostics(failure: &Self::UnmountFailure) -> &FailureDiagnostics;
}
impl<'n, 'o, 'w> StorageLayout for Volume<'n, 'o, 'w, Mounted> {
    type Unmounted = Volume<'n, 'o, 'w, Unmounted>;
}
impl<'n, 'o, 'w> PreparationStorage for Volume<'n, 'o, 'w, Mounted> {
    type MountFailure = Volume<'n, 'o, 'w, Unmounted>;
    fn mount(
        volume: Self::Unmounted,
        parent: &Path,
        _deadline: Instant,
    ) -> Result<Self, (FailureDiagnostics, Self::MountFailure)> {
        volume.mount_ext4(parent).map_err(|failure| {
            let (source, volume) = failure.into_parts();
            (source.into(), volume)
        })
    }
    fn rollback_mount(
        failure: Self::MountFailure,
        _deadline: Instant,
    ) -> Result<Self::Unmounted, Self::UnmountFailure> {
        Ok(failure)
    }
    fn pin(&self, recovery: &RecoveryDirectory) -> Result<Vec<FilesystemPin>, StoragePinFailure> {
        self.pin(recovery)
            .map(|pin| vec![pin])
            .map_err(StoragePinFailure::from)
    }
    fn validate_command_root(&self) -> io::Result<()> {
        self.validate_command_root()
    }
    type UnmountFailure = VolumeUnmountFailure<'n, 'o, 'w>;
    fn unmount_until(self, _deadline: Instant) -> Result<Self::Unmounted, Self::UnmountFailure> {
        // The preparation state machine checks its original deadline before and
        // after each transition, including a single-volume unmount or retry.
        self.unmount()
    }
    fn retry_unmount(
        failure: Self::UnmountFailure,
    ) -> Result<Self::Unmounted, Self::UnmountFailure> {
        failure.retry()
    }
    fn unmount_diagnostics(failure: &Self::UnmountFailure) -> &FailureDiagnostics {
        failure.diagnostics()
    }
}
impl<'n, 'o, 'w> StorageLayout for BootstrapVolumes<'n, 'o, 'w, Mounted> {
    type Unmounted = BootstrapVolumes<'n, 'o, 'w, Unmounted>;
}
impl<'n, 'o, 'w> PreparationStorage for BootstrapVolumes<'n, 'o, 'w, Mounted> {
    type MountFailure = Box<BootstrapMountFailure<'n, 'o, 'w>>;
    fn mount(
        volume: Self::Unmounted,
        parent: &Path,
        deadline: Instant,
    ) -> Result<Self, (FailureDiagnostics, Self::MountFailure)> {
        volume
            .mount_ext4_until(parent, deadline)
            .map_err(|failure| (failure.diagnostics().latest_only(), failure))
    }
    fn rollback_mount(
        failure: Self::MountFailure,
        deadline: Instant,
    ) -> Result<Self::Unmounted, Self::UnmountFailure> {
        failure.rollback_until(deadline)
    }
    fn pin(&self, recovery: &RecoveryDirectory) -> Result<Vec<FilesystemPin>, StoragePinFailure> {
        let tool = self.tool().pin(recovery).map_err(StoragePinFailure::from)?;
        match self.output().pin(recovery) {
            Ok(output) => Ok(vec![tool, output]),
            Err(failure) => {
                let mut failure = StoragePinFailure::from(failure);
                failure.pins.push(tool);
                Err(failure)
            }
        }
    }
    fn validate_command_root(&self) -> io::Result<()> {
        self.tool().validate_command_root()
    }
    type UnmountFailure = Box<BootstrapUnmountFailure<'n, 'o, 'w>>;
    fn unmount_until(self, deadline: Instant) -> Result<Self::Unmounted, Self::UnmountFailure> {
        self.unmount_until(deadline)
    }
    fn retry_unmount(
        failure: Self::UnmountFailure,
    ) -> Result<Self::Unmounted, Self::UnmountFailure> {
        failure.retry()
    }
    fn unmount_diagnostics(failure: &Self::UnmountFailure) -> &FailureDiagnostics {
        failure.diagnostics()
    }
}

/// Pin acquisition failure owns every already-published identity and any empty
/// target. Preparation releases these before unmounting either filesystem.
#[derive(Debug, thiserror::Error)]
#[error("storage identity pin: {source}")]
pub struct StoragePinFailure {
    pub(super) source: io::Error,
    pins: Vec<FilesystemPin>,
    target: Option<FilesystemPinTarget>,
}
impl From<FilesystemPinFailure> for StoragePinFailure {
    fn from(failure: FilesystemPinFailure) -> Self {
        let (source, target) = match failure {
            FilesystemPinFailure::Before(source) => (source, None),
            FilesystemPinFailure::Mount { source, target } => (source, Some(target)),
        };
        Self {
            source,
            pins: Vec::new(),
            target,
        }
    }
}
impl StoragePinFailure {
    pub(super) fn into_cleanup(
        self,
        namespace: crate::isolation::UserNamespace<crate::isolation::PinnedNamespace>,
    ) -> (io::Error, crate::step::pins::PinCleanup) {
        let mut cleanup = crate::step::pins::PinCleanup::namespace(namespace);
        for pin in self.pins {
            cleanup = cleanup.with_filesystem(pin);
        }
        if let Some(target) = self.target {
            cleanup = cleanup.with_target(target);
        }
        (self.source, cleanup)
    }
}
