// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{
    Active, Created, Mapping,
    loop_device::{LoopDevice, ReadOnly},
};
use crate::cache::PreparedScute;
use std::{io, num::NonZeroU64};

/// Read-time verified data, with its loop devices and backing retained. No raw
/// control or writable backing handle escapes. Drop requests deferred mapping
/// removal and loop autoclear; only close confirms successful release.
#[derive(Debug)]
pub(crate) struct VerityMapping<'s, 'w> {
    mapping: Mapping<Active>,
    inputs: Inputs<'s, 'w>,
}

#[derive(Debug)]
struct Inputs<'s, 'w> {
    hash: LoopDevice<ReadOnly>,
    data: LoopDevice<ReadOnly>,
    prepared: PreparedScute<'s, 'w>,
}

impl<'s, 'w> PreparedScute<'s, 'w> {
    pub(crate) fn activate(self) -> io::Result<VerityMapping<'s, 'w>> {
        let inputs = Inputs {
            data: LoopDevice::<ReadOnly>::attach(self.files().0)?,
            hash: LoopDevice::<ReadOnly>::attach(self.files().1)?,
            prepared: self,
        };
        let target = inputs
            .prepared
            .header()
            .builder(
                inputs.data.id()?,
                inputs.hash.id()?,
                inputs.prepared.root().to_vec().into_boxed_slice(),
            )?
            .build();
        let mapping = Mapping::<Created>::create()?.activate(
            NonZeroU64::new(target.data_sectors()).expect("validated nonempty extent"),
            &target,
            true,
        )?;
        Ok(VerityMapping { mapping, inputs })
    }
}
impl<'s, 'w> VerityMapping<'s, 'w> {
    pub(super) fn snapshot_header(&self) -> io::Result<devmap_snapshot::Header> {
        use std::os::unix::fs::FileExt;
        let mut record = [0; 16];
        self.mapping
            .open_read_only()?
            .read_exact_at(&mut record, 0)?;
        let header = devmap_snapshot::Header::decode(record)?;
        if !header.valid || header.chunk_size.bytes().get() != 4096 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported or invalidated scute header",
            ));
        }
        Ok(header)
    }
    /// Diagnostic identity, not an independent lifetime lease.
    pub(crate) fn id(&self) -> devmap_linux::DevId {
        self.mapping.id()
    }
    pub(crate) fn close(self) -> io::Result<()> {
        self.mapping.close()?;
        self.inputs.hash.close().map_err(|(error, device)| {
            drop(device);
            error
        })?;
        self.inputs.data.close().map_err(|(error, device)| {
            drop(device);
            error
        })?;
        Ok(())
    }
}
