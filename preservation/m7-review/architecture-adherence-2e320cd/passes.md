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
