# Responsibility and simplification review

Reviewed source: `7b2f6b9d074eea443060b7ae2288600b1b803a96`.
Scope: architectural review of the responsibility inventory, with Claude Code,
followed by two rounds of source-grounded challenge and revision.
Claude explicitly agreed to the final constraints and conditional prototype in
round 3. The order below prioritizes the direct-gate prototype before a broader
cancellation refactor so their changes do not overlap unnecessarily. No production changes or new
test runs. This is a proposal, not validation of an implemented replacement.

## Constraints

The VM kernel and supervisor are trusted; recipe commands are untrusted. Keep
commands separated from supervisor/control descriptors, raw block devices,
context, cache and output publication. Preserve namespace-root mapping, capability
and seccomp restrictions, SELinux labeling, ID/ACL validation, filesystem
confinement, finite execution timeouts, fixed setup/teardown deadlines, and
cgroup cancellation plus reaping/draining before storage release. Explicit normal
unmount/writeback and healthy snapshots remain prerequisites for successful
sealing. Failed cleanup is terminal; unresolved resources remain retained for VM
disposal. Preserve recipe, COPY, firmware, transform, OCI and inventory behavior.
Keep safe Rust, methods, RAII and useful ownership/typestate transitions.

## Inventory corrections

The 16,070-line baseline is a conservative source count excluding test modules,
comments and blanks, but including some individually gated test/fixture methods.
Per-responsibility allocations are estimates, not measured disjoint partitions.
Sharing a source file does not establish double counting. Claude's initial
comparison of physical lines against these code allocations was withdrawn.
Removing test-only helpers must be reported separately from reducing code that
is compiled into the production binary. No new reduction was measured here.

Explicitly identify these previously buried responsibilities:

- Trusted-helper role dispatch and argument encoding/validation:
  `src/execution/helper.rs`.
- Carrying the original absolute monotonic setup cutoff across helper re-exec:
  `src/step/cutoff.rs`.
- Verifying mount-namespace identity and attached control mounts before use or
  cleanup: `src/anchor/namespace.rs` and its callers.
- Validating firmware permission/capacity for a changed DTB:
  `src/firmware.rs::DtbContract`. Preserve signed-identity restrictions, recognized
  target checks, external-DTB fill requirements and unchanged opaque-firmware
  pass-through behavior.

Diagnostic relay and PID-1 reaping were already in the inventory. Retaining
handles is an ownership mechanism supporting multiple responsibilities, not a
separate externally visible feature.

## Existing process topology

Per command, a short-lived seed creates the mapped user namespace and is reaped.
Then supervisor -> receiver -> namespace-entry helper -> restricted PID-1 init
-> image shell. Bootstrap additionally runs a trusted formatter helper outside
this command chain. COPY executes through trusted confined filesystem code.

The receiver retains initial-namespace privileges for mounted-view/resolver
cleanup and hosts diagnostic relay threads. Namespace entry is single-threaded
and irreversible; the next child becomes PID 1. The restricted init reaps image
processes and holds only a status endpoint beyond standard descriptors. Keep
this topology for the first simplification pass. These are necessary roles in
the present design, not proof that no other valid design could use fewer helpers.

## Proposed order

### 1. Test the production transports; remove shadow implementations

`CommandSetup::receive` in `src/isolation/setup.rs`, device channel methods in
`src/isolation/devices.rs`, and `IdentityReceiver`/`IdentitySender` plus
`Process::launch` are test-only transports beside the real production paths.
Tests using those shadows do not prove the production decoder accepts/rejects
identical packets.

Keep thin stdin entry points and expose internal socket-taking methods used by
both production and tests. Migrate tests to the actual gated setup, identity and
formatter parsing. Delete shadow transports only after preserving their useful
assertions against production paths. This is principally a coverage improvement;
do not book test-source removal as production-binary simplification.

Preserve message shapes/opcodes, descriptor count/order, truncation rejection,
CLOEXEC, ancillary type rejection, bounded waits, queued-message requirements,
and post-receive namespace/filesystem/socket/resolver checks. Rootless malformed
packet and descriptor-lifetime tests are the focused acceptance gate.

### 2. Consolidate narrow IPC and deadline mechanics

Targets: `src/process/identity.rs`, `src/process/supervision/joining.rs`,
`src/isolation/setup/gated.rs`, `src/isolation/gate.rs`,
`src/formatter/input.rs`, and repeated remaining-time conversions.

Use methods on small private types only if they replace more code than they add.
Keep protocol schemas and object-type checks at their owning call sites.
Distinguish already-queued one-shot datagram receives from deadline-bounded
sequence-packet exchanges and externally supervised waits. A shared helper must
not turn WouldBlock into a retry where the current contract requires a queued
packet, accept unexpected descriptors, or introduce an unbounded wait.

A deadline conversion helper may share arithmetic; every existing checkpoint,
phase boundary and original absolute cutoff stays. `SetupCutoff` continues to
handle the cross-process clock representation. Never renew a deadline in a helper.

Claude's transport/deadline estimate is roughly 150–175 net production lines,
uncertain and unmeasured. This is a hypothesis to verify with a patch, not a
promised saving. Test real framing/error behavior rootlessly and use relevant
existing VM normal, network/offline, timeout, gate-failure and missing-cleanup
cases for changes that affect live transport/wait behavior.

### 3. Prototype direct supervisor-to-init command control separately

Today the receiver relays readiness, admission/start and shell completion between
`ExecutionGate` and `Supervision`. Candidate: the supervisor owns the gate end;
the restricted init gets the same limited endpoint through trusted setup. The
receiver keeps its privileged cleanup responsibilities and its separate control
channel.

The receiver's successful cleanup report can include the separately observed
namespace-helper exit status. The supervisor compares it against the raw shell
status received from restricted init using the current exit/signal normalization
rule (`src/isolation/receiver.rs:227`). This preserves two observations without
adding a channel; it does not turn a cleanup report into shell success.

Hard requirements: containment before admission; full restriction and label/fd
checks before READY; execution timer established before START; original setup
cutoff and late-send/ack handling retained; no command inherits control endpoints;
missing/failed cleanup always rejects and still cancels/drains; queued reports and
peer-death races cannot be mistaken for success; receiver teardown/diagnostic
failure remains a build failure; normal unmount and snapshot checks stay.

Prototype only. Reject if maintaining these properties requires restoring the
same relay machinery or adds complexity. Validate normal/nonzero/signal outcomes,
status disagreement, peer loss at transitions, timeout, gate failure, blocked
descendants, held/missing cleanup and descriptor confinement. Do not claim savings
or implementation safety before that evidence exists.

### 4. Simplify terminal cancellation ownership

The current path wraps errors successively in `DomainFailure`,
`DomainCancelFailure`, `HelperCancelFailure`, and further reaping/drain failures.
`src/domain/helper.rs::HelperCancellation::finish_until` immediately unwraps them
for a private linear cleanup sequence.

Explore direct private fallible cleanup methods under the existing owning
lifetime, keeping successful transitions explicit and retaining unresolved
resources on failure. Do not replace several small types with a giant enum or a
mutable collection of optional resources. Do not add a generic leak helper merely
to rename `mem::forget`.

Acceptance requires a smaller, clearer concrete implementation plus unchanged
cancellation, reaping, descendant drain, deadline and retention behavior under
existing failure fixtures. Source inspection alone does not establish this.
No defensible net LOC estimate exists before that patch.

## Rejected shortcuts

- Reuse one user namespace across steps: same mappings do not prove equivalent
  namespace isolation. Shared networking is a separate explicit environment
  contract. Keep fresh user namespaces; no new qualification detour justified.
- Generic iterator over borrowed roots for unmounting: unmount consumes ownership,
  partial failures must retain other mounts, and sibling backing mounts are not
  the receiver's nested /mnt views. Current single/pair storage abstraction stays.
- Collapse Step into ContainedHelper wholesale: Step also owns storage, network,
  namespace and reaper/cancellation obligations. Compilation alone cannot prove
  cleanup ordering. Remove only demonstrably redundant type plumbing; audit all
  callers before considering the synthetic 'lease parameter redundant.
- Remove a resource-carrying field because it has no accessor: the rollback field
  also owns a RecoveryLease (directory lock and namespace lifetime).
- Replace all retention sites with one generic wrapper: different owners have
  different descriptor, kernel-resource and destructor effects.
- Merge filesystem path walkers indiscriminately: COPY, resolver scaffolding and
  captured-context traversal have distinct semantics and authority requirements.
- Remove seccomp, capabilities, label probes, status/cleanup distinctions or
  integrity checks to meet an LOC target.

## Review record

Local full prompts and responses are preserved in
`/storage/conglobate-validation/responsibility-review/` (`prompt.txt`, `round1.*`,
`challenge.txt`, `round2.*`, `final-challenge.txt`, `round3.*`). Claude session:
`04645b5e-6cbd-4f86-8ec7-6644984eafa2`.

Initial findings are not approvals: the later responses withdraw the mixed-metric
counts and unsafe/unsupported simplification arguments. No source changes, builds,
privileged experiments or new qualification results were part of this review.

Final accounting correction: 16,070 is the previously measured conservative
source baseline. The functional allocations and proposed savings, not that total,
are estimates. Claude's final response occasionally calls the total an allocation;
that wording is not adopted here. Likewise its ~90-line test-only deletion figure
is not independently measured and is not counted as a production reduction.
