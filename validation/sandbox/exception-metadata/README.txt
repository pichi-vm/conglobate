Authenticated exception metadata: userspace characterization
=============================================================
Reproduce without root, devices or mounts:
  python3 validation/sandbox/exception-metadata/run.py --out /tmp/NEW-exception-metadata

The safe-Rust helper imports two nonzero 4096-byte raw blocks, checks generated
exception records and changes one 64-bit metadata field per variant. It seals
and hashes the modified bytes, packages the correct digest/root, then invokes
actual Chain::stage. No assemble call or kernel exposure occurs. Each fixture
is 16 KiB. Observed at euid 1000 with a 10-second execution bound.

Controls: unchanged input and arbitrary old_chunk on a zero-new_chunk terminator
both authenticate unchanged. Further cases also authenticate unchanged: a new
chunk pointing at metadata, a chunk past the COW file, an origin index at the
64-GiB limit, duplicate origin and duplicate data chunk. Duplicates are recorded
as noncanonical cases, not presumed invalid under every kernel interpretation.
Evidence: reproduced/exception-metadata, including exact sources and dependency
parser snapshots. No generated images/binaries are retained in the repository.

Source explanation: Chain::stage checks blob digests, verity parameters/root and
ancestry; it does not traverse exception records. Pinned carapace assembly reads
and validates the snapshot header before constructing the snapshot target. The
snapshot Exception::decode wire API intentionally accepts all bit patterns and
puts bounds/layout checks on callers. Local trim_cow checks some new-chunk bounds
only for kernel-written writable output; it is not the incoming-chain validator.

Conclusion: authentication does not establish valid snapshot structure. These
results are NOT proof that kernel assembly accepts malformed exceptions or that
publication succeeds. Kernel rejection, read/error handling and cleanup of these
cases remain unqualified. The VM kernel remains trusted by the design, but this
fixture cannot qualify its input parser. Decide where semantic validation belongs
before claiming a userspace rejection boundary; do not duplicate an image-level
validator in the wire-format crate. This experiment changes no acceptance policy.
