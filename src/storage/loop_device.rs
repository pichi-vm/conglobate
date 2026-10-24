// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    marker::PhantomData,
};

mod sealed {
    pub(crate) trait Sealed {}
}
pub(super) trait Access: sealed::Sealed {
    const READ_ONLY: bool;
    fn flush(file: &File) -> io::Result<()>;
}
#[derive(Debug)]
pub(super) struct ReadOnly;
#[derive(Debug)]
pub(super) struct Writable;
impl sealed::Sealed for ReadOnly {}
impl sealed::Sealed for Writable {}
impl Access for ReadOnly {
    const READ_ONLY: bool = true;
    fn flush(_: &File) -> io::Result<()> {
        Ok(())
    }
}
impl Access for Writable {
    const READ_ONLY: bool = false;
    fn flush(file: &File) -> io::Result<()> {
        file.sync_all()
    }
}

/// Storage-private loop authority. Failed/deferred close returns the same owner;
/// autoclear on descriptor drop is not a successful-detachment receipt.
#[derive(Debug)]
pub(super) struct LoopDevice<A: Access> {
    device: lodown::Device,
    _access: PhantomData<A>,
}
impl<A: Access> LoopDevice<A> {
    pub(super) fn attach(file: &File) -> io::Result<Self> {
        let length = file.metadata()?.len();
        if length == 0 || !length.is_multiple_of(4096) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "loop backing must contain a positive number of 4096-byte blocks",
            ));
        }
        let device = lodown::Device::open(lodown::Control::open()?.get_free()?)?;
        device.configure(
            file,
            4096,
            lodown::Configurable {
                read_only: A::READ_ONLY,
                writable: lodown::Writable {
                    autoclear: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        )?;
        Ok(Self {
            device,
            _access: PhantomData,
        })
    }
    pub(super) fn id(&self) -> io::Result<devmap_linux::DevId> {
        self.device.as_ref().metadata()?.try_into()
    }
    #[cfg(feature = "test-fixtures")]
    pub(super) fn number(&self) -> io::Result<u32> {
        Ok(self.device.status()?.number)
    }
    pub(super) fn close(self) -> Result<(), (io::Error, Self)> {
        let result = (|| -> io::Result<()> {
            A::flush(self.device.as_ref())?;
            lodown::Device::from(self.device.as_ref().try_clone()?).clear()?;
            match self.device.status() {
                Err(error) if error.raw_os_error() == Some(6) => Ok(()), // ENXIO: unbound
                Err(error) => Err(error),
                Ok(_) => Err(io::Error::new(
                    io::ErrorKind::ResourceBusy,
                    "loop detach remains deferred by a live holder",
                )),
            }
        })();
        result.map_err(|source| (source, self))
    }
}
impl Read for LoopDevice<Writable> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.device.read(bytes)
    }
}
impl Write for LoopDevice<Writable> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.device.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        Writable::flush(self.device.as_ref())
    }
}
impl Seek for LoopDevice<Writable> {
    fn seek(&mut self, offset: SeekFrom) -> io::Result<u64> {
        self.device.seek(offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_backing_lengths_fail_before_loop_control_access() {
        let backing = tempfile::tempfile().unwrap();
        // Invalid inputs only: this test never requests a kernel loop device.
        for length in [0, 1, 4095, 4097, 8191] {
            backing.set_len(length).unwrap();
            for error in [
                LoopDevice::<ReadOnly>::attach(&backing).unwrap_err(),
                LoopDevice::<Writable>::attach(&backing).unwrap_err(),
            ] {
                assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{length}");
                assert!(error.to_string().contains("4096-byte blocks"));
            }
        }
    }
}
