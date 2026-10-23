// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::Scratch;
use std::io::{self, Read, Seek, SeekFrom, Write};

/// Owns both the kernel loop attachment and its writable RAM backing. No backing
/// access or raw FD escapes while attached. Device closes before backing on Drop;
/// autoclear retains the kernel backing reference until remaining holders close.
#[derive(Debug)]
pub struct LoopAttachment<'workspace> {
    device: lodown::Device,
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
        let attempt = (|| -> io::Result<lodown::Device> {
            let length = self.file.metadata()?.len();
            if length == 0 || !length.is_multiple_of(4096) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "loop backing must contain a positive number of 4096-byte blocks",
                ));
            }
            let control = lodown::Control::open()?;
            let device = lodown::Device::open(control.get_free()?)?;
            device.configure(
                &self.file,
                4096,
                lodown::Configurable {
                    writable: lodown::Writable {
                        autoclear: true,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )?;
            Ok(device)
        })();
        match attempt {
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
        self.device.as_ref().metadata()?.try_into()
    }
    /// Diagnostic node number, not a lifetime lease or command authority.
    pub fn number(&self) -> io::Result<u32> {
        Ok(self.device.status()?.number)
    }
    /// Flush and confirm unbound state before returning writable file authority.
    /// clear() alone can mean deferred detachment, so it is not sufficient.
    pub fn detach(self) -> Result<Scratch<'w>, LoopDetachFailure<'w>> {
        let detached = (|| -> io::Result<()> {
            self.device.as_ref().sync_all()?;
            let duplicate = self.device.as_ref().try_clone()?;
            lodown::Device::from(duplicate).clear()?;
            match self.device.status() {
                Err(error) if error.raw_os_error() == Some(6) => Ok(()), // ENXIO: unbound
                Err(error) => Err(error),
                Ok(_) => Err(io::Error::new(
                    io::ErrorKind::ResourceBusy,
                    "loop detach remains deferred by a live holder",
                )),
            }
        })();
        match detached {
            Ok(()) => {
                drop(self.device);
                Ok(self.backing)
            }
            Err(source) => Err(LoopDetachFailure {
                source,
                attachment: self,
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
        self.device.as_ref().sync_all()
    }
}
impl Seek for LoopAttachment<'_> {
    fn seek(&mut self, offset: SeekFrom) -> io::Result<u64> {
        self.device.seek(offset)
    }
}
