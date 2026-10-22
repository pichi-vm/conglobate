Process-limit failure through the actual chain pipeline
=======================================================

Reproduce:
  python3 validation/sandbox/guest.py chain-pids-limit \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-CHAIN-PIDS-LIMIT --run

The real committed/locked carapace Chain/Snapshot fixture runs the mapped FUSE
worker and restricted entry helper. A trusted controller assigns the gated
namespace init to a cgroup with pids.max=1 before allowing image code to execute.
The supervisor and filesystem worker remain outside that limited cgroup. This
is test-environment resource policy, not conglobate provisioning requirements.

The bounded C test command verifies mapped UID0/PID1, writes and closes a partial
image file, then attempts fork exactly once. EAGAIN produces an explicit marker
and exit125. Unexpected fork success also exits unsuccessfully but lacks the
required marker, so cannot pass the fixture. No process proliferation loop or
host cgroup is involved. The controller checks a pids.events max event, empty
cgroup after completion and successful group removal.

Observed: command EAGAIN and status125 propagate through worker teardown to the
chain helper's pre-sealing gate. That probe intentionally panics with status101
on worker failure; it is not a proposal for production error handling. No sealed
or reconstructed/published output is produced (host verifier rejects success
markers). Base SHA256 stays unchanged; all FUSE/DM/loop resources disappear.
Guest exits zero. Source snapshots, kernel/runtime hashes, command and worker
build inputs accompany evidence in reproduced/chain-pids-limit.

Limits: one environment-provisioned process limit and one handled command
failure. This does not establish whole-build memory/FD/request exhaustion,
worker or supervisor exhaustion, a command ignoring failure and returning zero,
resource races before registration, or two simultaneous builds. Production must
retain explicit failure semantics; resource provisioning remains environmental.
