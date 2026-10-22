Snapshot creator death at ownership transitions
==============================================
Reproduce from repository root:
  python3 validation/sandbox/guest.py snapshot-death \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --out /tmp/NEW-snapshot-death --run

The runner copies the current Cargo manifest/lock and src into its output
folder, inserts six safe-Rust test-only stop gates into that copied snapshot.rs,
adds a deterministic lease-read gate to copied mounts.rs, and adds helper.rs as a binary. The instrumentation builder never patches production files. Build command,
source/binary hashes and compiler output are saved. The source catalog also
records the original production source and instrumentation generator.

A disposable guest creates a 64GiB logical zero origin, pins it, and requests
its deferred removal. The child enters a private mount namespace and calls
actual Scratch::new and instrumented Snapshot::create, including actual sparse
65GiB COW creation and autoclearing loop attachment. No host disks/shares/NIC.
The selected hook records its stage and stops itself. The observer verifies
stopped state, exact stage and empty underlying private-tmpfs lease, then sends
SIGKILL through a pinned pidfd and reaps that child. The parent's origin pin is
released and two fresh helpers invoke Scratch::new recovery.

Boundaries, immediately after:
- attached: COW loop attachment, before target/control creation;
- created: DM device creation, before Snapshot owns its rollback fields;
- loaded: inactive table load;
- resumed: table activation, before opening its device;
- opened: opening the writable device, before deferred removal;
- deferred: successful deferred removal ioctl, before setting the Rust flag.

Every case requires dead lease removal and no DM devices or attached loops.
Child reach/stop has an 8-second deadline, reaping/recovery 5 seconds, automatic
resource cleanup 3 seconds; the runner also bounds whole-guest lifetime.
No broad force removal is used to manufacture passing cleanup results.

Evidence: validation/sandbox/reproduced/snapshot-death. All six boundaries pass.
This tests actual creator death at explicit instrumented transitions. It does
not enumerate every possible instruction interleaving, table-load syscall
interruption, all recovery interleavings, hostile trusted-directory
mutation, full build restart, or filesystem crash consistency. There is no FUSE
worker or recipe in this focused test. Real COW writes/persistence and held
writable handles are separately covered by reproduced/cow-recovery. Carapace's
separate pre-resume activation defect remains unresolved in the pinned library.

Concurrent recovery and vanished-lease regression
-------------------------------------------------
Current execution starts two recovery helpers stopped before Scratch::new. The
parent owns the real .conglobate-scratch.lock, resumes both, and confirms both
are blocked in flock(LOCK_EX) on that exact inode via /proc/PID/syscall and fd
metadata. This observation uses x86_64 syscall 73, matching the fixture target.
It then unlocks and requires both helpers to finish successfully. All six creator
death boundaries pass with no remaining dead lease or storage dependency.

Initial concurrent run failed with ENOENT. Source analysis found recovery could
open a live lease directory before its owner removes it, then acquire the lock
after that owner closes it, and finally read_dir the vanished pathname. The
production fix in src/mounts.rs skips only NotFound at this lookup; other errors
still fail. Existing mode/identity/locking checks remain.

A deterministic copied-source read gate stops after lease locking but before
the emptiness lookup. The parent removes the empty fixture lease and resumes
recovery. The exact former read_dir expression (test-only negative branch)
returns failure; the current path succeeds. This recreates the disappearance
condition independently of scheduler timing. It does not prove the exact syscall
sequence of the first unscripted failure, whose log contains only ENOENT.

Evidence: reproduced/snapshot-concurrent-recovery-negative (initial failure),
snapshot-concurrent-recovery-initial-fix, and snapshot-concurrent-recovery
(deterministic control plus all six concurrent cases). Original single-recoverer
evidence remains in reproduced/snapshot-death. Twelve library tests also pass.
Two contenders and these boundaries are bounded evidence, not exhaustive
concurrency proof, malicious trusted-directory protection or full build restart.
