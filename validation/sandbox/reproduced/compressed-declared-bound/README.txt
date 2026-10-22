Declared compressed-scute bound enforced during staging
======================================================
Actual repository fix, using a small bounded nonprivileged input regression.

Before: a valid zstd frame decodes to32768 bytes while authenticated descriptor
metadata declares2 blocks (8192 bytes). Staging writes all32768 bytes to its
private data file, then rejects the length mismatch. The new regression fails
with left32768/right8192 on the retained before-scute.rs source. This demonstrates
late enforcement, not a large decompression/resource-exhaustion experiment.

Fix in src/scute.rs: validate nonzero/checked declared block extent, cap decoding
writes at that extent, then read one decoded byte to require EOF. Extra data
fails before writing beyond the declaration; decoder trailer/checksum errors
propagate. Final block-count/padding and authenticated-root checks remain. Salt,
ancestry and transport digest validation are not replaced by the length bound.
Optional data_blocks remains optional: this change adds no arbitrary limit to
undeclared decoded streams, which remain subject to configured scratch resources.
It bounds written output, not every internal decoder allocation or CPU operation.

Evidence:
- New32768->8192 regression passes after fix, with no hash file produced on error.
- Existing8-case compressed-stream matrix now runs both with and without a
  declaration: valid single/split frames, invalid magic/header, truncated/wrong
  checksum, trailing junk and appended valid-frame data. Exact-extent reads still
  validate stream completion; appended data fails at declaration before hashing.
- Existing extent/padding matrix covers absent declarations, partial final block,
  undersized/oversized declarations, zero and u64::MAX. All non-root scute tests
  pass (6); the2 kernel/DM tests are explicitly ignored in this host run.
-8 artifact tests pass, including cancellation and publication collision checks.
- Strict clippy for lib/scutes/artifacts, cargo fmt --check and diff check pass.

Strict linting initially found a64KiB stack allocation in artifact copying plus
existing artifact-test item placement/nested-if style findings. Buffer now uses
heap storage with the same64KiB read size; test changes only reorder declarations
and collapse equivalent conditions. Intermediate lint failures are retained with
final passing logs. No unrelated repository changes were reset or committed.

Reproduce:
 cargo test --locked --test scutes --test artifacts
 cargo clippy --locked --lib --test scutes --test artifacts -- -D warnings
 cargo fmt --check

This closes the identified declared-output staging bound. It is not general
zstd parser qualification, a policy for undeclared expansion, snapshot exception
semantic validation, new kernel test coverage or the final sandbox executor.
Source snapshots, before/after results and commands are retained in this folder.
