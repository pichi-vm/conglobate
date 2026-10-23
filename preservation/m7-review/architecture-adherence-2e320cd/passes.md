# Corrected architecture adherence

Frozen baseline: 2e320cd; exact document and source-anchor hashes in scope.json.
These are parent self-reviews, not independent reviewers or repeated test runs.
Claude's full review and terminal consensus are supplemental evidence. Milestone
scope is the M2 architecture contract; no implementation qualification is implied.

## Pass 1 — requirements-first — CLEAN (1/3)

Read the complete corrected ARCHITECTURE and reconciled it with DESIGN's trust,
CLI/input, stage/schema, transform, lifecycle, firmware and inventory contracts,
and SANDBOX's storage, isolation, network, cleanup and qualification requirements.
The latest decisions are coherent: environment-owned fixed exact-name exception,
no registration, dedicated disposable VM and independent shutdown, private mapped
command roots, no command SYS_ADMIN, explicit input paths, local verified inputs,
cache-backed immutable layers and direct zero-backed bootstrap. Recipe execution
and artifact/attestation integration boundaries remain distinct.

Architecture defines actual modules and owners, not extra proposed abstractions.
BuildPlan owns private immutable descriptions and exposes reads; Volume and Step
use sealed marker traits with state-specific resources. Views borrow mounted
owners and namespace-sensitive handles remain thread-affine. Private fields and
crate-private descriptor access establish the public API boundary without claiming
that Rust proves external kernel behavior. Meaningful owning types, consuming
transitions, no unsafe application code, no utility-function architecture and
explicit fallible completion remain required by the architecture.

Followed the corrected failure distinction through StepState resources,
CommandResult, ExecutionOutcome and StorageExecutionFailure::accept: helper
status and command outcome are separate; finalization alone cannot grant command
success. Failed setup cannot pass acceptance. Teardown retains exact resources
and original deadline on failure; quiescence precedes namespace release and
storage finalization. CompletedImage publication follows private context cleanup
and validated inventory; decoded inventory alone grants no execution authority.
Drop and VM shutdown are not normal-success receipts.

Filesystem schema and formatter profile agree on ext4, 4096 alignment, capacity
bounds, label encoding and immutable effective arguments/config identity.
Inventory schema remains version two, deterministic strict CBOR with explicit
resolver exclusion and input/output identities; architecture delegates the exact
wire fields to DESIGN rather than inventing another schema. Pipeline descriptions
preserve all inherited components, disposable tool ancestry, firmware pairing,
CARAPACE and authored/default transform distinctions.

Inspected current positive/negative VM assessment and status records: successful
two-root bootstrap versus denied second-root admission with cleanup/no output.
Neither claims full sandbox or distro qualification. Traceability explicitly
requires missing receiver/legacy-fault/network/distro/platform coverage; those
are implementation gates, not hidden architectural omissions. Latest rootless
logs remain supporting evidence, not additional adherence passes. No tests rerun
merely to manufacture a pass.

Preservation checks: both archived architecture/M4 source SHA-256 records verify;
full-review and receiver-review gzip streams read successfully. Large artifacts
remain explicitly durably_archived=false; M8 cannot retire them yet. Historical
acceptance is marked superseded, current M2 is reopened, and no old source or
validation evidence was removed. Terminal Claude consensus confirms corrections,
retracts F3 and leaves no disputed architecture finding.

No findings in this corrected scope. Clean streak 1/3. Two further full passes
with different starting points are required before M2 acceptance. M4-M8 remain
unaccepted and no qualification gap is waived.

## Pass 2 — failure/publication-first — CLEAN (2/3)

Rechecked every frozen document/source hash before reviewing; all unchanged.
Started at publication and worked backward through inventory, context cleanup,
finalization, descendant drain, admission and partial resource acquisition.
Reviewed those boundaries against the complete architecture's ownership/error,
pipeline, schema, traceability and dedicated-VM requirements and the normative
contracts already reconciled in pass 1. This is a second parent self-review,
not Claude's consensus counted twice and not another test execution.

StagedDirectory explicitly disclaims successful-build authority. It anchors
publication to a parent FD and uses no-replacement rename; a failed rename keeps
the staging owner. CompletedImage remains private and couples finalized image
and inventory; Pipeline closes private context before publishing. Inventory
construction identifies captured recipe/lock bytes, builder and actual output;
wire decoding alone grants no authority. This matches the architecture's separate
raw-storage, completed-output and measured-execution boundaries.

Backward resource trace: successful command outcome is required after successful
storage finalization. Finalization and teardown failure owners retain the exact
phase and original deadline; retry cannot admit or rerun a command. Quiescent
holds the drained helper, Finishing carries helper status, and command outcome
remains separately associated with cleanup. Namespace release happens after drain.
CommandNetwork owns its NAT/resolver resources through the command lifetime and
has explicit close/retry state. ContextCleanupFailure retains pinned parent and
remaining cleanup authority; partial deletion never reconstructs a read-only
checkout. Setup namespace-creation failure variants retain any started process,
reaper and assigned volume instead of inventing a completed state.

Rust/style review: meaningful owners and consuming transitions carry resources;
sealed Checkout/Volume/Step markers restrict valid transitions, while internal
runtime enums represent acquisition/recovery branches with different concrete
payloads. Read-only FailureDiagnostics shares error payloads without sharing
mutable authority. No new freestanding utility architecture, unsafe extraction,
mutable manifest bag or destructor-success premise is introduced by the design.
Public/private authority boundaries remain explicit; kernel reachability and
quiescence still require runtime validation.

Reconciled remaining M2 categories: filesystem capacity/profile and exact receipt
schema are specified, immutable planning and actual module boundaries remain
consistent, stage inheritance/tool-disposal/firmware pairing are unchanged, and
transform/network/CBOR contracts retain explicit defaults and exclusions. The
requirements-to-tests map distinguishes compiler/pure tests, current limited VM
runs, historical noncompiling harness coverage and unavailable platforms.

Preservation and claims: no reviewed file changed or reference retired; earlier
failed timeout and policy experiments remain evidence, not rewritten successes.
M8 still requires durable recovery of external bytes. M4-M8 and the three-pass
implementation gates remain open; architectural completeness does not substitute
for their tests. No new architecture findings. Clean streak 2/3; one full pass
remains before M2 acceptance.
