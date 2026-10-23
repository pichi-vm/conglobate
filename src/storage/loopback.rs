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
pub struct LoopAttachment<'workspace> {
    device: LoopDevice<Writable>,
    backing: Scratch<'workspace>,
}
#[derive(Debug, thiserror::Error)]
#[error("loop attachment: {source}")]
pub struct LoopAttachFailure<'workspace> {
    source: io::Error,
    backing: Scratch<'workspace>,
}
impl<'w> LoopAttachFailure<'w> {
    pub fn into_parts(self) -> (io::Error, Scratch<'w>) {
        (self.source, self.backing)
    }
}
#[derive(Debug, thiserror::Error)]
#[error("loop detachment: {source}")]
pub struct LoopDetachFailure<'workspace> {
    source: io::Error,
    attachment: LoopAttachment<'workspace>,
}
impl<'w> LoopDetachFailure<'w> {
    /// Terminal pipeline conversion: abandon retry authority, preserving the
    /// exact I/O cause. Drop/autoclear is not a successful detachment receipt.
    pub(crate) fn into_terminal_error(self) -> io::Error {
        let Self { source, attachment } = self;
        drop(attachment);
        source
    }

    pub fn source_error(&self) -> &io::Error {
        &self.source
    }
    pub fn retry(self) -> Result<Scratch<'w>, Self> {
        self.attachment.detach()
    }
}
impl<'w> Scratch<'w> {
    /// One attempt. Another claimant can win after get_free; kernel EBUSY is
    /// reported with the backing intact. No blocking advisory lock/retry loop.
    pub fn attach(self) -> Result<LoopAttachment<'w>, LoopAttachFailure<'w>> {
        match LoopDevice::<Writable>::attach(&self.file) {
            Ok(device) => Ok(LoopAttachment {
                device,
                backing: self,
            }),
            Err(source) => Err(LoopAttachFailure {
                source,
                backing: self,
            }),
        }
    }
}
impl<'w> LoopAttachment<'w> {
    pub fn id(&self) -> io::Result<devmap_linux::DevId> {
        self.device.id()
    }
    /// Diagnostic node number, not a lifetime lease or command authority.
    pub fn number(&self) -> io::Result<u32> {
        self.device.number()
    }
    /// Flush and confirm unbound state before returning writable file authority.
    /// clear() alone can mean deferred detachment, so it is not sufficient.
    pub fn detach(self) -> Result<Scratch<'w>, LoopDetachFailure<'w>> {
        let Self { device, backing } = self;
        match device.close() {
            Ok(()) => Ok(backing),
            Err((source, device)) => Err(LoopDetachFailure {
                source,
                attachment: Self { device, backing },
            }),
        }
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
