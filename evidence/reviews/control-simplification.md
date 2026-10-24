# Control-path simplification

Baseline: `7b2f6b9`. Implements the source-grounded consensus recorded in
[the responsibility review](responsibility-simplification.md).

The trust boundary and process topology are unchanged. The supervisor now owns
init's READY/START/status gate directly. The receiver forwards its endpoint over
its existing channel, retains mount/resolver/diagnostic cleanup, and reports its
independently reaped namespace helper exit with the cleanup acknowledgment.
The supervisor checks this exit against the normalized shell status. A shell
report never substitutes for cleanup, cgroup quiescence or writeback.

`ContainedHelper::finish_until` closes control, cancels the cgroup, reaps the
helper and drains adoptees in order under one cutoff. It replaces the nested
cancellation/reaping/draining retry owners. Failed cancellation retains the
process; failed reaping retains the wait owner; the enclosing Step retains
namespace, network and mounted storage until cleanup succeeds or the VM dies.
No failed build returns reusable storage. Ready/Running and mount states remain.

`DescriptorPacket` shares only one nonblocking send/receive and descriptor
ownership. Callers still enforce framing, exact descriptor shapes and object
types. Queued datagrams fail immediately on missing input. Bounded exchanges
retry under their original deadlines; externally supervised process waits remain
explicit. `Deadline` shares remaining-time conversion, not timeout policy.

Tests now exercise production gated-setup, formatter and supervision decoders.
The old identity/device/ungated-setup test transports were deleted. This portion
improves coverage and removes test support; it is not production-binary savings.

## Adherence review

The direct-gate and terminal-cancellation changes were reviewed together because
removing admission transitions changes which objects own cancellation. They are
one implementation milestone, not separately qualified partial states.

1. Trust and failure-path source review: shell control descriptors remain
   CLOEXEC and audited out of image children; only trusted init holds the status
   endpoint. READY follows restrictions and labeling probes. START follows
   deadline creation. Receiver cleanup and independently observed helper status
   remain required. Observation errors still cancel/drain. No namespace reuse,
   kernel permission expansion, extra supervisor process or new runtime channel.
2. Ownership and Rust-style source review: methods belong to resource/protocol
   owners; safe Rust, sealed states and consuming transitions remain. No generic
   leak wrapper or mutable lifecycle enum. Unfinished Step storage remains
   retained; domain failure cannot lead to helper reaping receipts or sealing.
   Cancellation failure retains process ownership before propagating its error.
3. Protocol and acceptance review: exact descriptor count/type, truncation,
   CLOEXEC, EOF and wrong-phase checks remain at production decoder seams.
   Fixed setup and teardown cutoffs are not renewed, and late start/completion
   checks remain. Status disagreement rejects cleanup. Missing acknowledgment
   still attempts cancellation/drain, then rejects finalization. Artifact,
   recipe, COPY and inventory code is unchanged.

No source issue remained in these three consecutive self-reviews. Test and
external-review evidence below is a separate acceptance gate.

## Validation

Evidence directory: `/storage/conglobate-validation/control-simplification/`.
All privileged checks run inside owned disposable VMs, never on the host.

Rootless suite: 210 tests. The count differs from 213 because seven obsolete
receiver-admission tests became four direct-gate/cleanup tests; a deadline test
and the decoder migration had already balanced the earlier shadow-test removal.
The production control test separately exercises mismatched helper exit and
normalized signaled-shell status. Clippy uses `--locked --all-targets -D warnings`.

The historical `sandbox-gate-failure` case now injects missing completion after
START while the receiver retains its mounts and control channel. It verifies
cleanup and terminal busy-unmount retention. It no longer claims to exercise a
receiver-local START failure: that relay was removed. A separate `sandbox-start-reject` VM fixture now rejects START after READY
with the namespace helper blocked, then verifies cancellation, full descendant
drain and root unmount. Hosted CI runs both gate cases, held-cleanup and
missing-cleanup fixtures on both architectures.

Local VM assessments passed: contained helper cleanup, held cleanup, missing
cleanup, cleanup timeout, lost completion plus busy unmount, START rejection,
and writeback/snapshot failure. The network build and independent artifact
verifier passed DNS/TCP, restricted capabilities, namespace reuse, read-only
resolver injection/restoration and the following offline step. This fixture does
not qualify external NAT. The full pipeline build and artifact verifier passed
bootstrap/firmware tool exclusion, one carapace scute, DTB root binding and
inventory/blob-graph verification. These are build checks, not new distro boots.

Source-size audit uses the same script on baseline and current source, excluding
all unit-test modules and fixture roots. The broader test-module matcher corrects
the prior baseline of 16,070 to 16,057 code lines (13 diagnostic-test lines were
previously included). Current: 15,315 code lines, 742 fewer (4.62%). Physical
source falls from 17,721 to 16,904. This conservative measure still includes
individually gated test helpers, so the 742-line reduction is not all production
runtime code. The transport-test cleanup is reported separately above; no logic
was moved to another crate. Script, filtered sources and JSON are retained with
the evidence.

Hosted CI passed for implementation commit `ecca738`: [rootless/dependency
checks](https://github.com/pichi-vm/conglobate/actions/runs/40097885754) and
[disposable VM lifecycle](https://github.com/pichi-vm/conglobate/actions/runs/40097885750).
Both amd64 and arm64 passed, including the four added gate/cleanup cases. ORAS
interoperability passed on amd64. Job receipts are saved with the local evidence.

Two intermediate unit runs failed. One unchanged resolver-path test returned
EAGAIN; the error was not suppressed or changed into success. A separate gate
test assumed dropping a socket immediately closes every inherited copy, which
is false while another parallel test spawns before exec. That test now explicitly
shuts down the peer before checking rejection, without sleeps. The completed
suite passes all 210 tests. Failed logs are retained as `unit-resolver-eagain.log`
and `unit-start-close-race.log`. Ancillary truncation coverage includes a 32-FD
offer rejected by the production formatter decoder, with every right released.

The independent Claude implementation review and any follow-up are retained in
this evidence directory alongside the source-grounded planning consensus. This
document's three adherence passes are self-reviews, not claims that Claude
performed those passes.


## Independent implementation review

Claude found no trust or functionality regression and concluded the agreed plan
is satisfied. Its source audit independently traced containment, restriction,
START timing, separate status corroboration, terminal retention and descriptor
checks. The reviewer ran no tests; validation results above are our evidence.

We accepted the stale supervision-comment finding and migrated the remaining
rootless START tests from a test-only wrapper to `start_reporting_until`, removing
the wrapper. A suggested status-frame defect was withdrawn by the reviewer after
tracing the length guard; we added a comment explaining that precondition.
The claim that ARCHITECTURE still named `Step<Cancelling>` was stale: that document
had already been updated. Ready/Running deliberately remain distinct markers:
they share cleanup resources but gate different execution/teardown operations;
`RunningCommand` additionally owns `ExecutionGate<Started>`.

The new START-rejection VM test satisfies the reviewer's topology concern.
Mismatched cleanup status is covered rootlessly, not by an additional VM fixture;
the receiver is trusted and the actual error path still requires cancellation.
No additional runtime feature or security exception was introduced for review.
