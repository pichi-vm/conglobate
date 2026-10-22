Command memory limit through real chain failure handling
========================================================

Reproduce:
  python3 validation/sandbox/guest.py chain-memory-limit \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-CHAIN-MEMORY-LIMIT --run

This extends chain-pids-limit with a different bounded command resource failure.
Committed/locked inputs, actual Chain/Snapshot, mapped FUSE and restricted entry
remain in use. The environment controller sets the limit before image execution.

The trusted controller assigns the gated command to memory.max=64MiB with
memory.swap.max=0. After closing a partial image write, the command makes one
128MiB allocation and touches pages through volatile writes. The controller
requires memory.events oom_kill >= 1; the worker failure must be exit137.
An allocation error or normal return does not satisfy the expected markers.
The supervisor/worker are outside this memory cgroup, and no host limit changes.

Observed: real command failure reaches the worker-status gate before sealing.
No reconstructed or published output exists; success markers are forbidden by
the host verifier. Base SHA256 is unchanged, cgroup is empty/removable and all
FUSE/DM/loop resources disappear. Guest exits zero. Probe-helper panic101 is a
test failure gate, not the proposed production error API.

Scope: command-local resource failure, not supervisor or filesystem-worker
exhaustion, all simultaneous pressure combinations, or command code ignoring
an error and returning success. Resource policy belongs to the environment.
C fixtures are test-only, bounded and executed solely in the disposable guest.
No production executor or maintained-backend qualification is implied.

Evidence: reproduced/chain-memory-limit includes source snapshots, guest tool/
kernel hashes, command/output and experimental worker build catalog.
