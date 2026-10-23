# M4 storage ownership review subset

Baseline 1a6b226. Partial self-review, not a whole-milestone adherence pass.

SANDBOX-DESIGN 3.1–3.4 and 10.3–10.5:
- Workspace creates detached 0700 root-owned tmpfs. Scratch uses anonymous
  O_TMPFILE, borrows workspace, has no public descriptor export; no disk fallback.
- PreparedScute keeps aligned uncompressed cache data as the same open inode.
  It generates verification metadata and compares the authenticated root before
  activation. Compressed/partial-final-block inputs materialize in RAM only.
  Later kernel reads go through VerityMapping, not the original data loop.
- Verity inputs retain prepared data/tree plus read-only autoclear loops.
  Mapping activation errors retain acquired resources in explicit cleanup
  variants. Header reads occur through the verified mapping before snapshot
  assembly; header validity and 4096-byte chunks are checked.
- ReadOnlyChain stacks snapshot mappings over dm-zero and verified COWs.
  Explicit close and fallback Layers drop run top-down. Produced extensions
  validate parent-root identity and retain sealed RAM backing separately.
- Writable snapshots consume their backing loop and borrow the origin. Busy
  mapping close retains origin and backing. Loop detach flushes and requires
  unbound status before exposing writable backing again. Deferred removal
  and autoclear do not create successful finalization receipts.
- Volume mount uses fixed nodev/nosuid policy and a pinned device FD. Failed
  activation returns the original unmounted owner. Explicit unmount syncs
  first; busy failures retain mount/volume owners. Lazy detach is fallback
  only and cannot return an Unmounted value.
- Runtime attachments borrow the workspace; parent ownership/mode, namespace
  identity and empty control directories are checked. Explicit close uses a
  retained deadline; abandonment leaves mounted resources for VM disposal.

Evidence gap encountered: current maintained VM tests did not include the old
prepare-scutes case. Migrated its assertions and synthetic generator, retaining
the original files. Current cache-storage-v2 passes in disconnected native-amd64
VM. It verifies cache-backed versus materialized choices, read-only verified
reads, busy cleanup with retained diagnostics, EIO on mutation after activation
before first read, rejection on re-preparation, and retained inode identity
after path replacement. All modifications are to a private synthetic fixture
inside the guest; no host cache is changed. v1 passed the same assertions but
used the old generator due to import order; that migration error is corrected
and preserved, not silently discarded. Strict fixture Clippy passes.

No new production defect established in this subset. This is not a complete
M4 source pass or qualification of the full chain/boot/publication path.
Preparation rollback, containment/recovery records, end-to-end deadlines and
complete evidence reconciliation still require review. Hashes identify read
sources, not a claim of complete scope coverage.
