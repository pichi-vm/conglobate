// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{
    Scratch,
    loop_device::{LoopDevice, Writable},
};
use std::io::{self, Read, Seek, SeekFrom, Write};

/// Owns both the kernel loop attachment and its writable RAM backing. No backing
/// access or raw FD escapes while attached. Device closes before backing on Drop;
/// autoclear retains the kernel backing reference until remaining holders close.
#[derive(Debug)]
pub(crate) struct LoopAttachment<'workspace> {
    device: LoopDevice<Writable>,
    backing: Scratch<'workspace>,
}
impl<'w> Scratch<'w> {
    /// One attempt. Another claimant can win after get_free; kernel EBUSY is
    /// reported with the backing intact. No blocking advisory lock/retry loop.
    pub(crate) fn attach(self) -> io::Result<LoopAttachment<'w>> {
        let device = LoopDevice::<Writable>::attach(&self.file)?;
        Ok(LoopAttachment {
            device,
            backing: self,
        })
    }
}
impl<'w> LoopAttachment<'w> {
    pub(crate) fn id(&self) -> io::Result<devmap_linux::DevId> {
        self.device.id()
    }
    /// Diagnostic node number, not a lifetime lease or command authority.
    #[cfg(feature = "test-fixtures")]
    pub(crate) fn number(&self) -> io::Result<u32> {
        self.device.number()
    }
    /// Flush and confirm unbound state before returning writable file authority.
    /// clear() alone can mean deferred detachment, so it is not sufficient.
    /// A shared BSD lock held by a udev probe is drained first. An opener that
    /// does not hold that lock still fails this call.
    pub(crate) fn detach(self) -> io::Result<Scratch<'w>> {
        self.device.close().map_err(|(source, device)| {
            drop(device);
            source
        })?;
        Ok(self.backing)
    }
}
impl Read for LoopAttachment<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.device.read(bytes)
    }
}
impl Write for LoopAttachment<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.device.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.device.flush()
    }
}
impl Seek for LoopAttachment<'_> {
    fn seek(&mut self, offset: SeekFrom) -> io::Result<u64> {
        self.device.seek(offset)
    }
}
