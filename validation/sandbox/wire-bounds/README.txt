Snapshot wire arithmetic bounds, distinct from semantic validation
=================================================================
Pinned devmap2679426fa1d4508fc5c3bbdf643efbdbe64a3edd; tracked source checkout
matches that revision. Testing used a disposable copy, not a dependency edit.
Cargo.lock records the copied workspace's exact dependency resolution. Three
additional pure-record tests plus the two upstream wire tests pass offline.

Coverage:
- All22 supported power-of-two chunk sizes512bytes..1GiB decode and round-trip.
- Zero, non-power-of-two, above-constraint and overflowing sector-to-byte inputs
  reject with InvalidData. No buffer is allocated from those declared sizes.
- For each supported chunk size, the last representable metadata-area index
  succeeds and the next rejects with InvalidInput. Independent u128 arithmetic
  checks the result. No seek, device access or large memory allocation occurs.
- Extreme exception indices including u64::MAX round-trip. This is intentional:
  Exception is a wire record, and its caller owns extent/layout validation.
- Upstream wire tests retain header magic/version/validity/chunk checks and the
  ignored old_chunk field of a zero-new_chunk terminator.

Reproduce from this retained folder:
 cargo test --locked --offline --manifest-path sources/Cargo.toml \
  -p devmap-snapshot --test wire --test validation_bounds \
  --target-dir /tmp/NEW-wire-bounds-target
The offline dependency versions must be cached. sources/ is the exact copied
workspace with only tests/validation_bounds.rs added to snapshot. The original
workspace file identities are in source-catalog.json; the added test is also at
validation/sandbox/wire-bounds/bounds.rs in the main repository. No production
API was changed, and no kernel test is implied by this host-only result.

Reconciliation with prior evidence:
Chain::stage authenticates bytes/ancestry; carapace validates its required
snapshot header before constructing the snapshot target; kernel assembly and
reads interpret exception mappings. Prior exception-kernel/README.txt records
nine bounded cases including deferred EIO, metadata-as-data, duplicates and
missing termination. None establishes a universal semantic admission validator.
Do not add one to devmap-snapshot's intentionally thin wire API or treat successful
wire decoding as proof of valid filesystem contents. An explicit image-admission
policy and its implementation location remain design choices. The trusted-kernel
model is unchanged; these cases are not an exploit or kernel security claim.

This closes these finite wire arithmetic boundaries. It is not proof about all
kernel inputs, all snapshot semantics, filesystem-worker memory safety or the
final executor. Existing authenticated build and failure/cleanup evidence remains
separate, as does the generic recipe-execution integration requirement.
