Filesystem-worker OOM with pending command requests
==================================================

Reproduce:
  python3 validation/sandbox/guest.py chain-worker-oom \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-CHAIN-WORKER-OOM --run

The actual committed/locked Chain/Snapshot pipeline starts the mapped FUSE
daemon through a trusted stop-before-exec gate. An independent initial-root
controller assigns only that process to a memory cgroup, then resumes it.
This ensures its post-exec allocations are charged there. Commands, launcher
and supervisor remain outside the daemon's limited group.

Two restricted command writers must be live. The trusted driver stops the
daemon; the controller pins its FUSE abort FD and observes a queued request.
It verifies nonzero worker memory charge, lowers memory.max to zero and requires
memory.events oom_kill >= 1. The controller does not send SIGKILL to the daemon.
This is controlled resource-failure injection, not a natural workload benchmark.

After the kernel OOM kill, the driver signals/reaps the command init (which may
already have exited on filesystem errors). The controller aborts before waiting
for blocked tasks. Worker wait must report137; unmount completes. Worker wrapper
returns125, causing the real chain pre-sealing failure gate to reject output.
Controller survives and removes the empty cgroup. Base SHA256 stays unchanged;
all DM/loop/FUSE resources disappear. No sealing/reconstruction/publication
success marker is allowed by the host verifier. Cleanup completes within the
measured15-second bound; the guest has the outer150-second watchdog.

Limits: daemon-only cgroup OOM at one active-I/O boundary. Not system-wide OOM,
supervisor/guardian OOM, allocator errors returned without a kill, all transient
writeback failures, or a production cgroup-management implementation. Resource
policy remains environmental. A failed/aborted filesystem is never sealed even
if some prior writes completed. Experimental worker/backend adoption remains open.

Evidence: reproduced/chain-worker-oom includes source snapshots, guest command,
output, kernel/runtime hashes and worker build catalog.
