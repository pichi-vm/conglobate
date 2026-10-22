Current cases now use two inherited layers. See TWO-LAYER-REVIEW.txt for current
results, reproduction and limits; the original single-layer evidence follows.

Concurrent builds sharing block-backed base inputs
===================================================
Reproduce:
  python3 validation/sandbox/guest.py chain-shared-block \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-shared-block --run
Repeat with case chain-shared-block-failure and a different new output directory.

Both disposable guests have no NIC, host disk or host share. Initial root
imports one owned 64MiB ext4 fixture into a scute and attaches its data/hash files
to two read-only loop devices. Each independent helper stages/authenticates the
same base content using conglobate, compares its root to the prepared root, then
uses the pinned carapace API to assemble from the shared block handles. The
normal path without CHAIN_SHARED_BLOCK continues to use conglobate Chain::assemble.
This diagnostic override is confined to the validation helper, not production.

At the bounded worker gate, actual DM tables show two separate writable COW
snapshots whose origins are the same base snapshot mapping, one shared verity
mapping and one zero origin. Thus this exercises reuse of the full single-scute
base stack, not just the shared dm-zero seen in file-backed tests. Both commands
run in the existing restricted FUSE fixture; no block handle is given to them.

Success: release A while B remains mounted. A seals, reconstructs/verifies and
publishes its inventory; B stays active without output. Release B and require
its equivalent completion. Exact principal success markers occur twice. Base
raw copies and shared scute data/hash bytes are unchanged. DM mappings disappear;
the supervisor detaches the two explicitly owned shared input loops and checks
no remaining attached loops. Guest exits0.

Failure: A's existing benign fixture command returns7 after its ordinary writes.
Its helper exits101; no A verified/output path exists. B remains active and then
completes verification/publication. Per-build logs require no sealing/publication
in A, and one complete success in B. Shared bytes and both raw bases remain
unchanged, with the same full cleanup. Guest exits0 means these assertions pass,
not that the intentionally failed build succeeded.

Evidence: reproduced/shared-block-concurrent and shared-block-concurrent-failure,
including source/input catalogs and actual DM tables. Success-run exact counts
were independently checked before retention; future runner checks both cases.

Limits: one base scute, two builds, distinct writable snapshots, controlled
command failure, no simultaneous process death during activation. Both trusted
workers use the fixture's same mapped UID range; this is storage-lifetime
validation, not cross-build security or allocator proof. No hardware device,
multi-layer shared-block chain, final executor qualification, or resolution of
the separately demonstrated pre-resume death defect is claimed. Production
continues privately staging cache blobs; this tests the underlying block-input
assembly/ownership contract without changing that cache design.
