//! Bounded pure-record checks; no allocations based on declared geometry or I/O.
#![forbid(unsafe_code)]
use devmap_snapshot::{Exception, Header};

fn record(sectors: u32) -> [u8; 16] {
    let mut bytes = [0; 16];
    for (field, value) in bytes.chunks_exact_mut(4).zip([0x7041_6e53, 1, 1, sectors]) {
        field.copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[test]
fn all_supported_chunk_sizes_and_rejected_conversion_bounds() {
    for power in 0..=21 {
        let sectors = 1_u32 << power;
        let header = Header::decode(record(sectors)).unwrap();
        assert_eq!(header.chunk_size.bytes().get(), sectors * 512);
        assert_eq!(header.encode(), record(sectors));
    }
    for sectors in [0, 3, 1 << 22, 1 << 23, u32::MAX] {
        assert_eq!(Header::decode(record(sectors)).unwrap_err().kind(),
                   std::io::ErrorKind::InvalidData);
    }
}

#[test]
fn last_representable_metadata_index_and_first_overflow() {
    for power in 0..=21 {
        let header = Header::decode(record(1 << power)).unwrap();
        let stride = u128::from(header.chunk_size.bytes().get()) / 16 + 1;
        let last = (u128::from(u64::MAX) - 1) / stride;
        let index = u64::try_from(last).unwrap();
        assert_eq!(u128::from(header.metadata_chunk_index(index).unwrap()), last * stride + 1);
        assert_eq!(header.metadata_chunk_index(index + 1).unwrap_err().kind(),
                   std::io::ErrorKind::InvalidInput);
    }
}

#[test]
fn exception_wire_decode_does_not_claim_semantic_validation() {
    for (old_chunk, new_chunk) in [(u64::MAX, u64::MAX), (u64::MAX, 0), (0, u64::MAX)] {
        let exception = Exception { old_chunk, new_chunk };
        assert_eq!(Exception::decode(exception.encode()), exception);
    }
}
