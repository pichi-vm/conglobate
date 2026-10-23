// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Terminal storage diagnostics. Borrowing recovery owners cannot escape the
//! build workspace. These conversions preserve concrete causes, then relinquish
//! retry authority with the same Drop behavior as the former string conversion.
//! They never certify cleanup; the dedicated VM must be disposed after failure.
use super::BuildError;
use crate::storage::{
    LoopAttachFailure, LoopDetachFailure, MappingFailure, ReadOnlyChainFailure, SealFailure,
    SnapshotMappingFailure, VolumeMountFailure, VolumeUnmountFailure,
};

impl From<MappingFailure> for BuildError {
    fn from(failure: MappingFailure) -> Self {
        Self::Mapping(failure.diagnostics().clone())
    }
}
impl From<SnapshotMappingFailure<'_, '_>> for BuildError {
    fn from(failure: SnapshotMappingFailure<'_, '_>) -> Self {
        Self::Snapshot(failure.diagnostics().clone())
    }
}
impl From<ReadOnlyChainFailure<'_, '_>> for BuildError {
    fn from(failure: ReadOnlyChainFailure<'_, '_>) -> Self {
        Self::ReadOnlyChain(failure.diagnostics().clone())
    }
}
impl From<VolumeMountFailure<'_, '_, '_>> for BuildError {
    fn from(failure: VolumeMountFailure<'_, '_, '_>) -> Self {
        let (source, volume) = failure.into_parts();
        drop(volume);
        Self::VolumeMount(source)
    }
}
impl From<VolumeUnmountFailure<'_, '_, '_>> for BuildError {
    fn from(failure: VolumeUnmountFailure<'_, '_, '_>) -> Self {
        Self::VolumeUnmount(failure.diagnostics().clone())
    }
}
impl From<LoopAttachFailure<'_>> for BuildError {
    fn from(failure: LoopAttachFailure<'_>) -> Self {
        let (source, backing) = failure.into_parts();
        drop(backing);
        Self::LoopAttach(source)
    }
}
impl From<LoopDetachFailure<'_>> for BuildError {
    fn from(failure: LoopDetachFailure<'_>) -> Self {
        Self::LoopDetach(failure.into_terminal_error())
    }
}
impl From<SealFailure<'_>> for BuildError {
    fn from(failure: SealFailure<'_>) -> Self {
        let (source, backing) = failure.into_parts();
        drop(backing);
        Self::Seal(source)
    }
}

#[cfg(test)]
mod tests {
    use super::BuildError;
    use crate::{firmware::ExtractionError, storage::FailureDiagnostics};
    use std::io;

    #[test]
    fn firmware_extraction_and_storage_cleanup_remain_separate() {
        let extraction = ExtractionError::Io(io::Error::from_raw_os_error(2));
        let cleanup = FailureDiagnostics::from(io::Error::from_raw_os_error(16))
            .followed_by(io::Error::from_raw_os_error(5).into());
        let combined =
            BuildError::from(extraction).with_cleanup(Err(BuildError::Snapshot(cleanup)));
        let BuildError::OperationAndCleanup { operation, cleanup } = combined else {
            panic!("independent failures erased");
        };
        let BuildError::Firmware(ExtractionError::Io(source)) = *operation else {
            panic!("extraction cause erased");
        };
        assert_eq!(source.raw_os_error(), Some(2));
        let BuildError::Snapshot(history) = *cleanup else {
            panic!("snapshot cleanup history erased");
        };
        assert_eq!(history.operation().raw_os_error(), Some(16));
        assert_eq!(history.cleanup()[0].raw_os_error(), Some(5));
    }
}
