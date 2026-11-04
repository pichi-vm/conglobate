// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use zerocopy::{
    FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned, byteorder::little_endian::U32,
};

/// One legacy MBR slot; CHS addresses do not determine GPT discovery.
#[derive(Debug, FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
#[repr(C)]
pub(super) struct MbrEntry {
    pub(super) boot_indicator: u8,
    pub(super) start_chs: [u8; 3],
    pub(super) os_type: u8,
    pub(super) end_chs: [u8; 3],
    pub(super) start: U32,
    pub(super) size: U32,
}

const _: () = assert!(size_of::<MbrEntry>() == 16 && align_of::<MbrEntry>() == 1);
const _: () = assert!(std::mem::offset_of!(MbrEntry, start) == 8);

impl MbrEntry {
    pub(super) fn protects(&self, sectors: u64) -> bool {
        self.os_type == 0xee
            && self.boot_indicator == 0
            && self.start.get() == 1
            && u64::from(self.size.get()) == (sectors - 1).min(u64::from(u32::MAX))
    }
}
