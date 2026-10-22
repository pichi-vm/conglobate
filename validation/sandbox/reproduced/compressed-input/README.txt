Authenticated compressed-input validation
=========================================
Actual non-root Chain::stage receives small bounded blobs with correct OCI
digests/sizes in every case. Two controls (one checksummed zstd frame and two
concatenated frames splitting identical plaintext) recover the exact original
bytes and authenticated root. Invalid zstd magic, three-byte header truncation,
missing checksum byte, flipped checksum byte and trailing non-frame junk return
decoder I/O errors. An additional valid frame carrying extra data passes decoding
but fails the authenticated-root match. data_blocks is deliberately omitted to
exercise root binding rather than the earlier declared-extent check. The staged
source bytes are checked against the entire supplied blob.

Full tests/scutes.rs run: five non-root tests pass; two kernel tests explicitly
ignored, not counted here. No production code changed. Source/lock hashes and
exact command accompany results. These cases validate streaming decode/error
propagation and decoded-root binding, not arbitrary zstd parser safety, resource
exhaustion, exception-record validation or generic recipe execution.
