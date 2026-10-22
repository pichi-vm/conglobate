Live-build admission and outer publication composition
======================================================
Diagnostic coordinator and fixed real FUSE build, not production adoption.

Run both chain-build-admission and chain-build-cancel:
 python3 validation/sandbox/guest.py CASE \
  --kernel-version 7.2.10-200.fc44.x86_64 \
  --worker-build /tmp/conglobate-finalize-workers --out /tmp/NEW-CASE --run

The safe-Rust arbiter's optional --await-completion mode starts at Building.
reserve/commit cannot advance it. A trusted prepare request changes Building to
Prepared; cancellation changes Building or Prepared to Cancelled and prepare
cannot undo it. Existing default mode still starts Prepared for earlier tests.
The control channel remains trusted: prepare ASSERTS completion, not a public
API that independently validates a build. No recipe can authorize it via logs.

admission.py starts this arbiter before the build in a protected mode0700
/work/admission-control directory outside command roots. The build runs through
the existing safe-Rust ownership-init handoff, trusted PID init and actual
Chain/Snapshot/FUSE/restricted-command/Record pipeline. At a bounded rendezvous
while the command is running, reserve and commit must both return Building and
public output must be absent. A trusted worker relay bridges only this scheduling
rendezvous across its private FUSE mount. The rendezvous never establishes
completion eligibility. The ordinary command still checks /work is unavailable.

Success: release command, require successful dedicated launcher status (it reaps
namespace init), require terminal launcher pidfd, and require no DM/loop residue.
Namespace init's existing final check rejects live descendants and reaps exited
children. Move the completed INTERNAL artifact into the protected coordinator
directory; then prepare/reserve/commit. Public bytes must match staging hashes.

Cancellation: accept cancellation during the command rendezvous, then deliberately
let the actual build finish. Apply the same completion/resource checks. Even with
a valid complete staged artifact, prepare/reserve/commit must all return Cancelled,
public output must remain absent, and internal artifact hashes must be unchanged.
This proves accepted cancellation cannot be undone by later successful completion.
It is NOT a prompt cancellation/teardown test; earlier teardown fixtures remain
separate. The complete internal artifact is intentionally retained for inspection.

Both integrated cases pass, including base preservation, filesystem verification,
successful worker finalization and descendant checks. The arbiter is outside the
build lifetime namespace and is reaped through an owned pidfd after assertions.
Existing post-completion arbitration has a separate regression run with this
modified helper. No production CLI, inventory format or runtime dependency added.

Retained initial failure: v1 attempted to observe the rendezvous inside the
worker's FUSE mount from the outer mount namespace. The command timed out with18,
the owner failed, and the guest exited1. No public commit occurred. v2 adds the
bounded trusted relay and retains the failed source/log catalog as negative
fixture evidence; it is not a product defect or a passing cancellation test.

Remaining composition gates: prompt cancellation and owner death in this exact
coordinator topology, authoritative handoff on coordinator loss, shared identity
allocation/lifetime, combined resource pressure, recovery/orphan decisions and
generic executor integration. No comprehensive trusted-socket exclusion audit,
unbounded-client defense, storage power-loss durability or attestation claim.
In particular, a protected lease becoming available does not prove writer death.
