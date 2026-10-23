# Single-pass materialized scute preparation

Resolves performance-002. Aligned uncompressed layers remain cache-backed.
Compressed and partial-block inputs now stream through a private owning
MaterializingReader into the existing exact-length verity formatter. Decoded
bytes are written once; generated zero padding is supplied by ScuteExtent and
RAM file extension, not copied from untrusted input. No reread of materialized
data is required. The reader does not expose writable backing authority.

ScuteExtent rejects short/long input and propagates the final EOF read error.
DecodedBlob::finish still authenticates the encoded blob digest, checks decoder
completion and rejects leftover bytes. Both finish and trusted-root comparison
must succeed before returning PreparedScute. Any source, write, format, digest,
root, extension or sync failure drops private scratch owners; no prepared owner
escapes. Existing cache-backed inode/read-time verification is unchanged.

144 library tests and 13 cache tests pass; strict all-target Clippy passes.
New tests cover exact/partial extents, padding/root equivalence, short and long
inputs, late read errors and output write failure. Existing cache tests cover
invalid compressed checksum, truncation, trailing junk and blob digests.
Native amd64 KVM cache-storage passes aligned/partial/compressed data and zero
padding, read-only mappings, repeated cleanup failures and cache corruption
rejection. Source comparison records exact tested bytes. No performance timing
claim: the removed materialized-file reread is established by source/ownership.

This targeted review is not a complete M4 adherence pass. arm64/confidential
platform coverage and full qualification remain outstanding. Raw logs, source
archive and assessment are retained; large assets are locally hashed, not claimed
remotely durable.
