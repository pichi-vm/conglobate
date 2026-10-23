# Zstandard feature selection

At d6578fd, accepted finding dependencies-012 was still present: the direct
dependency enabled legacy decoders, fixed-array helpers and dictionary training
through its defaults. Production uses stream::read::Decoder; fixture encoders
use ordinary streaming compression. No caller uses array helpers or training.

Disabled default features without changing the locked dependency versions.
Before/after feature graphs show only std remains for zstd-safe/zstd-sys, both
in the root crate and the maintained lifecycle fixture dependency graph.
This deliberately removes legacy pre-1.0 decoding support; it does not change
the current Zstandard frame format used by our scute fixtures. Dictionary
training is distinct from decoding frames and is not used by this application.

128 library tests and 13 cache tests passed. Coverage includes mixed compressed
chains, decoded length checks, checksum corruption, truncation, trailing junk
and valid concatenated frames. No new test mirroring the manifest edit was added.
No binary-size, throughput, VM boot or cross-platform claim follows from this.

All-target Clippy found one leftover reference-to-reference at
StoragePreparationFailure's record cleanup transition from the previous path
borrowing refactor. Removed that redundant borrow; corrected all-target Clippy
passed. The previous lifecycle-package Clippy check did not lint the dependency
crate as the selected package. Both failed and successful logs are retained.

Before rebuilding, relocated the regenerable build cache to /storage after
checksum comparison. The original target path is a symlink; no validation or
/var/tmp evidence was removed. Relocation details and logs are included. This
local copy is not remote durable preservation.

Focused review only. M4 remains 0/3; no complete adherence pass is claimed.
