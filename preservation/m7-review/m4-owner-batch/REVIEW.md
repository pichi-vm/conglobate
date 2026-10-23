# Shared loop-device ownership review

Resolves simplification-009; this is a targeted review, not a complete M4
adherence pass. M4 remains at 0/3.

Both attachment paths now use private LoopDevice<A> with sealed access markers.
Configuration remains a single attempt, block size 4096 and autoclear enabled.
Only Writable implements Read/Write/Seek and syncs before detach. ReadOnly sets
the kernel read-only flag and exposes neither IO traits nor the raw descriptor.
No new public API or runtime state flags are introduced.

Consuming close confirms ENXIO after clear. A deferred detach is ResourceBusy,
not success; error paths return the same owning object. Writable backing is
returned only after confirmed detach; failed detach reconstructs the attachment
with its backing intact. Verity retains its existing staged cleanup order and
failure history. Field order still drops loop authority before backing storage.
Safe library APIs implement the kernel operations; no unsafe application code.

Validation:
- tests.log: 141 library tests pass.
- clippy.log preserves the initial unreachable-pub failure; sealed-trait
  visibility was corrected. clippy-final.log: strict all-target Clippy passes.
- chain-v1: deferred detach with an extra holder, retry after closure and
  writeback pass; verified-chain/snapshot/cleanup cases also pass.
- cache-v1: cache-backed scute preparation, read-time corruption rejection,
  owned verity reads and cleanup diagnostics pass.

chain: 149/151 compared files match
cache: 151/151 compared files match

The chain snapshot differs only in sealed-trait visibility (before the Clippy
fix) and incidental formatting of the unrelated network fixture (since reverted).
The cache snapshot matches final source. These native amd64 KVM tests do not
qualify arm64 or confidential computing platforms. Large assets remain at the
recorded local paths with hashes; no remote durability is claimed. Raw failures,
source archives, assessments and logs are retained alongside this review.
