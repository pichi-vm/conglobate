Coordinator-death composition with safe-Rust child ownership links
=================================================================
Reproduce cases chain-build-owner-death and chain-build-owner-unlinked:
 python3 validation/sandbox/guest.py CASE \
  --kernel-version 7.2.10-200.fc44.x86_64 \
  --worker-build /tmp/conglobate-finalize-workers --out /tmp/NEW-CASE --run

Both cases run the actual Chain/Snapshot/FUSE command pipeline while the external
publication arbiter is Building. A trusted coordinator pins namespace init and
records its own and its owned children's process identities at a bounded gate.
The outer observer pins those identities, checks PPid relationships and verifies
all are still nonterminal. These are trusted fixture records, not recipe output.
Only the coordinator is signaled on the linked passing path.

Negative control: plain Popen children (arbiter and dedicated launcher) survive
coordinator SIGKILL, as does the launcher's namespace init. All three pidfds remain
nonterminal after the bounded observation. The observer then explicitly signals
those owned actors as fixture cleanup. This is a reproduced lifetime defect in
the unlinked composition, NOT automatic teardown or a surviving-write claim.

Candidate: owned-exec.rs forbids unsafe Rust, arms PR_SET_PDEATHSIG(SIGKILL),
checks an original-parent pidfd supplied on stdin, replaces stdin with null,
constructs PATH/HOME/LC_ALL and execs the trusted child without changing credentials.
The original parent is the dedicated single-threaded coordinator. Both launcher
and arbiter execute through this helper. The existing ownership-init separately
links the whole-build PID namespace to the launcher. No external runtime adopted.

Observed candidate result: after coordinator SIGKILL, arbiter, launcher and init
pidfds become terminal within the fixture bounds. FUSE connections, DM targets
and loops disappear without observer abort/removal/cgroup kill or child signals.
Original base bytes are unchanged. Neither internal completed artifact nor
public output exists; runner rejects sealing/publication markers. GUEST_EXIT 0.
The arbiter lease/socket/staging directory is intentionally not garbage-collected.
Termination and cleanup evidence does not authorize arbitrary orphan deletion.

The observer reaps its coordinator child; terminal descendant pidfds establish
exit, not that this observer reaped nonchildren. No live-descendant claim is
inferred merely from parent exit or an available lease. The linked and negative
runs both retain source catalogs and exact results independently.

Scope: death while a real command is active, before reservation, in this exact
candidate topology. Prior isolated handoff tests cover other startup gates and
credential/thread limitations; they are not an exhaustive startup-failure matrix
for owned-exec. No credential changes or privileged exec are supported by this
wrapper contract. The creating thread must remain alive. Blocked-worker/cyclic
and combined-pressure variants, commit-phase coordinator loss, trusted-runtime
identity and maintained executor adoption remain separate. Do not generalize
these bounded results to uninterruptible kernel I/O or power-loss durability.
