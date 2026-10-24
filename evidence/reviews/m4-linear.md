# Focused M4 adherence reviews

Scope: production Rust at 9a70e50 (unchanged from 416ca11). Latest user decisions
and DESIGN.md/SANDBOX-DESIGN.txt supersede historical goal wording. These are
three self-reviews, not independent security certification. Existing evidence is
reused; no additional test campaign was run for this gate. External review
synthesis is recorded separately and must be resolved before closing the gate.

## Pass 1 — forward ownership and admission

Reviewed workspace/cache chain -> volume -> command preparation -> contained
helper -> restricted leaf -> command admission. Workspace owns anonymous tmpfs
scratch; verified immutable layers remain cache-backed. Volume's sealed states
and borrowed views keep mount ownership coupled to the snapshot. Preparation
retains the mapped namespace and checks owners/ACLs against separate supplied
UID/GID ranges before starting recipe code. Namespace entry closes transferred
handles; network entry precedes user-namespace entry. Private mount/PID/IPC/UTS
state, pivot/root preparation, capability and syscall restrictions precede the
leaf's behavioral label probes and readiness. Commands receive no context view.
No registration, network allocator or guardian was reintroduced. Owning methods,
private fields, consuming transitions and safe library calls retain the agreed
Rust style. Existing namespace/mount/join/launch/admission and confinement
fixtures exercise these paths; native-only evidence is not platform approval.
No new defect found. Preservation remains intact. Result: clean self-review.

## Pass 2 — reverse failure and publication exclusion

Read rejection -> finalized storage -> teardown -> cancellation/drain -> partial
setup. Runtime::scoped cannot return successful output alongside failed close.
StorageExecutionFailure can recover storage only for rejection/discard; cleanup
retry cannot mint ExecutedStorage. Step retains storage and mapped namespace
until cancellation, helper reap and descendant drain. Finalization validates
writeback/snapshot health under the original cleanup deadline. Original command
outcome survives cleanup failures and retries. Bootstrap discard attempts both
roots. Dedicated VM disposal remains the crash boundary; destructors are not
success receipts. Sealed states/RAII remain appropriate; the internal enums
represent retained failure ownership, not arbitrary externally mutable states.
Existing blocked-I/O/writeback/busy/held/missing/timed-out cleanup and supervisor
abort evidence covers these failure branches. No new test required. No new
defect found; no preservation loss. Result: clean self-review.

## Pass 3 — requirements and evidence challenge

Checked latest requirements against the source and maintained assessors: finite
recipe execution timeout (default five minutes), separate fixed setup/teardown
budgets; offline default; environment-owned enabled namespace/resolver; no
command SYS_ADMIN; exact-name external labeling policy behavior on both assigned
roots; no artifact downloads; cache-backed immutable data and RAM-backed mutable
storage. Source forbids unsafe Rust. Namespace/privilege/admission paths use
handshakes and bounded observation, not readiness sleeps. Existing tests verify
source identity, negative admission, absent output, cleanup, resolver restoration
and one scute per carapace stage. Hosted rootless and six-case lifecycle CI both
pass at 9a70e50; the earlier installation timeout is preserved, not counted as a
test failure or pass. Prior findings security-003/tests-001 retain Arm/platform
coverage limits; security-004 remains a distro compatibility assessment under
the trusted-kernel model, with no established M4 authority violation. M7 must
resolve qualification; M8 must establish durable archival before retirement.
No new defect or style violation found. Result: clean self-review.

## Disposition

Claude's bounded synthesis established no candidate defect, but its interrupted
broad review was incomplete and is not counted as a pass. I challenged its stale
network and thread-count concerns with current source; the bounded follow-up
confirmed both resolved and agreed that incomplete external coverage does not
veto the permitted self-review gate. See linear-lifecycle-consensus-final/REVIEW.md.
The intermediate follow-up hit its tool-turn limit; the final response used no
further tools. No additional tests were launched. M4 closes at three consecutive
clean self-reviews; this is not independent Claude approval or M7 qualification.
