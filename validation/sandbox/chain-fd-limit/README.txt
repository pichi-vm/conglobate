Command fd limit through real chain failure handling
========================================================

Reproduce:
  python3 validation/sandbox/guest.py chain-fd-limit \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-CHAIN-FD-LIMIT --run

This extends chain-pids-limit with a different bounded command resource failure.
Committed/locked inputs, actual Chain/Snapshot, mapped FUSE and restricted entry
remain in use. The environment controller sets the limit before image execution.

The trusted controller applies hard/soft RLIMIT_NOFILE=32 to the gated init.
The command verifies that limit, closes a partial image write, then makes at
most 40 open attempts on the existing image canary. EMFILE is required; held
descriptors are closed and the command exits125. No cgroup FD event is claimed:
the descriptor limit is verified by getrlimit and the actual EMFILE result.

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

Evidence: reproduced/chain-fd-limit includes source snapshots, guest tool/
kernel hashes, command/output and experimental worker build catalog.
