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
previously included). Current: 15,319 code lines, 738 fewer (4.60%). Physical
source falls from 17,721 to 16,907. This conservative measure still includes
individually gated test helpers, so the 738-line reduction is not all production
runtime code. The transport-test cleanup is reported separately above; no logic
was moved to another crate. Script, filtered sources and JSON are retained with
the evidence.

External implementation review and hosted CI acceptance follow below.
