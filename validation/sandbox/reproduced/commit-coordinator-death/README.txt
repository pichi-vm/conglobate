Coordinator death at reservation and output visibility
====================================================
Reproduce:
 python3 validation/sandbox/guest.py chain-commit-owner-death \
  --kernel-version 7.2.10-200.fc44.x86_64 \
  --worker-build /tmp/conglobate-finalize-workers --out /tmp/NEW-commit-owner --run

The actual fixed FUSE build produces a verified internal OCI artifact and Record
inventory. Safe-Rust namespace ownership, trusted-init descendant reaping and
outer DM/loop cleanup complete before this two-case matrix. A fresh protected
case directory receives a copy of that artifact. A dedicated single-threaded
coordinator starts the arbiter through owned-exec, supplying its own pinned pidfd.
The arbiter starts Building; trusted observer prepares/reserves only after the
completed build barrier. It pins owner/arbiter, verifies their parent relationship
and controls the two diagnostic timing gates. No recipe logs authorize commit.

Reserved gate: owner is killed through its pidfd after Reserved, before commit
is requested. Arbiter terminates via parent-death link without an observer signal.
No public output exists; staged artifact hashes remain identical.

Visible/unacknowledged gate: observer opens commit-no-reply, polls a separate
trusted status connection until Committed, and confirms no commit reply arrives.
It records the public inode and artifact hashes, then kills only the coordinator.
The linked arbiter becomes terminal; pending commit socket returns EOF, while
public inode and all artifact bytes remain unchanged. Staging has been renamed.
The source artifact remains unchanged in both cases. Each guest outcome marker
is present and GUEST_EXIT 0 appears exactly once.

Consequences: the lifecycle link removes the live arbiter after owner loss;
it does not undo visibility or turn lost acknowledgment into a no-output result.
This observer knows its controlled gate, but a restarted caller cannot inherit
that knowledge from absence of a reply. Protected record/recovery evidence remains
necessary; these in-memory statuses are not persisted attestations or receipts.

Scope: two defined publication boundaries, not a death injection inside the
rename/fsync syscall, a generic executor, autonomous orphan deletion or power-loss
durability. Earlier arbitration/record tests separately cover collision and
observed filesystem errors. This is combined owner-link and publication behavior,
not an exhaustive scheduler or syscall-atomicity proof. No public output is
removed to make the test pass. Case directories are left for guest inspection.

Initial packaging failed Errno122 while copying the executable, before QEMU.
preparation-error.txt records that non-runtime failure. Generated roots/boot files
listed in storage-cleanup.json were removed only after matching their retained
logs/input catalogs. The passing guest used a fresh output directory.
