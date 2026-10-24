# Architecture adherence review 1 — findings, streak 0/3

Scope: d5e77e6 source and architecture/design/goal ledger, including the current
one-scute-per-stage decision. Self-review, not independent review. This review
found contradictions; it is not a clean pass or a production qualification.

## Requirements and organization

Traced request -> CapturedBuild -> ResolvedBuild -> Pipeline -> StageOutput ->
CompletedImage. Captured bytes and cache verification precede privileged stage
execution. Pipeline owns stage order and sole publication. Firmware tool identity
is separate from base ancestry. StageOutput owns sealed/captured data, not live
mount/process resources. Per-stage sealing uses the full directive executor.
The environment owns Git preparation, fixed labeling policy, enabled networking
and dedicated-VM crash disposal; the binary owns command confinement and cleanup.

Findings fixed in this review:
1. REIMPLEMENTATION's milestone table still said M2/M3 complete at 3/3 despite
   the explicit reopenings at its top. Corrected both authoritative rows to 0/3.
2. Architecture publication and dependency descriptions still implied a context
   mount and teardown. Corrected to immutable captured bytes and runtime/workspace
   release; no second filesystem checkout exists.
3. Requirements-to-tests mapping omitted stage-level sealing and the maintained
   complete pipeline fixture. Added it with revision and coverage limits.

## Rust style, state and authority

Library and binary forbid unsafe code. Reviewed validated input values, sealed
VolumeState and StepState payloads, consuming Mounted/Unmounted transitions,
StageOutput data ownership and private CompletedImage publication. Production
entrypoint delegates; no utility-function framework was introduced by the linear
refactor. Shared read-only IDs/Arc<SealedScute> are distinct from writable mount
or process authority. Local branching over optional recipe stages is not a
persisted state machine. Runtime scope prevents returning runtime-borrowed owners.

RecoveryDirectory currently still owns a cooperative directory flock and pinned
control mount namespace. DomainTarget retains an Arc-backed lease; Volume borrows
the namespace authority. No journal or cgroup record is written. This vestigial
name/lock is a simplification candidate, but removing the namespace ownership
check along with it would be wrong. No claim that it implements cross-build
recovery, nor that the flock is a security boundary against privileged code.

## Failure, safety and evidence

Inspected Runtime::scoped's operation/cleanup outcome handling and the stage
executor's indexed failure ownership. Failed runtime cleanup cannot return stage
outputs to finalization. Stage failures retain/clean their storage and do not
seal a carapace. Step cancellation/reaping/drain is still mandatory after shell
exit. Drop is not a completion receipt; remaining uncertainty requires VM
shutdown. The namespace comparison is against acquisition, not PID 1.

Evidence: current 227 Rust tests/46 doctests, Clippy and complete-pipeline fixture
at 0556f30. Earlier lifecycle evidence is explicitly revision-scoped. Current
CI handles 40043887779 and 40043887757 were live when inspected, not yet passing.
Architecture can specify unqualified behavior without declaring M4–M7 complete;
current tests do not prove every failure/stage/platform combination.

## Preservation and remaining scope

No old implementation, validation data, failed run or user-untracked file was
removed. The new full-pipeline failure and successful run have source archives;
large assets remain local and cannot satisfy M8 durable recovery yet.

Remaining architecture choices are visible rather than silently assumed:
workspace mount is still binary-owned detached tmpfs; environment-owned workspace
was not adopted. UID/GID range remains the current fixed 65536-ID mapping, with
configurability unresolved. Full milestone qualification and cleanup remain open.
After the above documentation fixes, restart the three-pass sequence against the
updated scope. This file records findings, not advancement.
