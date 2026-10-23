# M4 adherence attempt at 168b2a2

Status: IN PROGRESS, no full pass awarded. This is the cumulative record for
this attempt; complete every remaining section before evaluating a clean pass.
The immediately preceding turn made implementation/test progress in 168b2a2.
No source change or new VM run occurred in this review segment.

## Requirements and authority

Latest user decisions override the original goal's policy-registration wording:
the environment supplies a fixed exact-security.selinux exception, and disposal
of the dedicated VM handles supervisor death. Conglobate does not register or
revoke policy. Normal success still requires complete process and filesystem
cleanup. Namespace/cgroup confinement and explicit fallible finalization remain
required. An environmental shutdown contract is not proof of deployed appliance
shutdown qualification.

## Segment A: cancellation, process ownership and failed preparation

Reviewed full source files listed in reviewed-source-sha256.json. Domain record
encoding/identity methods were also consulted, but the complete recovery/anchor
review is still pending and is not inferred from those excerpts.

1. Admission and cancellation are distinct capabilities. RecordedDomain launch
   retains domain ownership on launch/join failure. DomainJoin borrows the live
   domain, verifies its published record, and rechecks before exposing the join
   descriptor. ContainedHelper cancellation closes its channel before requesting
   cgroup cancellation. An admission acknowledgment failure can follow delivery;
   its failure owner permits cancellation, not a return to pre-admission state.
2. CommandDomain cancellation writes cgroup.kill, observes populated=0, then
   removes the owned directory. DomainWatch re-reads bounded cgroup.events after
   PRI notification; it does not use an always-ready IN poll or a sleep. Empty
   population is not treated as reaping or successful storage finalization.
3. HelperReaping collects the separately owned helper before DescendantDrain
   uses the process-wide ChildReaper. ChildReaper uses __WALL, maintains the
   exclusive wait prerequisite, checks its process/subreaper identity, and uses
   pidfd polling for live children. Its ECHILD receipt is only accepted after
   independent domain cancellation in the Step composition.
4. Process<Identified> signals through its pinned pidfd. Early unpaired helpers
   retain exclusive Child wait ownership; the documented normal-SIGCHLD and
   sole-waiter preconditions matter for numeric PID use. These are trusted
   supervisor process preconditions, not claims about arbitrary library callers.
   ChildLease Drop is bounded best effort; it produces no completion receipt.
5. Individual wait primitives can collect an already exited process, or observe
   an already empty domain, after a supplied cutoff. The Step/rollback/finalizer
   composition rechecks its fixed deadline at the next transition and before
   accepting completion. Therefore primitive success alone must not be cited as
   evidence of on-time lifecycle success. The current composed path retains its
   phase/storage on expiration.
6. Step states preserve namespace and storage through cancellation, reaping,
   draining and record cleanup. Only Finishing releases those owners. Mounted
   storage still requires finalization; StepCompletion is not ExecutedStorage.
   TeardownCompletion does not expose mounted storage through an into_parts API.
7. StoragePreparation failures retain the resources actually acquired: early
   helper wait owner, mount rollback, domain publication inspection, or assigned
   namespace/storage. Rollback follows consuming phase transitions under its
   original cutoff. Domain publication errors distinguish absent, exact and
   mismatched records; they never assume a failed publication wrote nothing.
   No recursive deletion of cgroup control files is used.
8. Finalization preserves the cutoff through unmount/retry, validates snapshot
   state, and checks time again before producing FinalizedStorage. Execution
   acceptance additionally requires successful primary outcome. Cleanup retry
   returns StorageExecutionFailure, with concrete error history retained; it
   cannot manufacture ExecutedStorage after a prior cleanup failure.
9. Diagnostic relay threads live in the outer trusted receiver. Pipe endpoints
   reaching command code provide diagnostic output only. Joining happens after
   namespace child exit and is bounded by the outer supervisor lifetime, rather
   than a second independent receiver timer. Detached relay fallback on local
   failure is not treated as completed cleanup; helper process exit/cancellation
   remains necessary.
10. Style: sealed state traits and owning transitions carry process/Step
    authority. Rollback phase enums model failure alternatives with concrete
    resources; they are not mutable acceptance flags. ManuallyDrop retention is
    deliberate because Drop cannot certify quiescence. FailureDiagnostics uses
    consuming history extension and read-only access. No new unsafe application
    code, free-standing orchestration function or sleep synchronization was
    found in this reviewed segment.

No new defect was established in this segment. This statement does not cover
unreviewed modules, unrestricted library consumers, or every acquisition race.

## Evidence fidelity checked in this segment

`evidence-source-comparison.json` reads preserved VM assessments and compares
all 18 reviewed source files against each executed source archive. No reviewed
file was missing from those archives. This is a subset comparison, not a claim
that an entire historical build matches current code.

- Older contained/publication/namespace/mount/launch/join/attached-exit and
  writeback runs match the reviewed domain, process, preparation and finalizer
  files. Differences in the selected set are the ObservedCommand re-export,
  rejection-only execution recovery, and Clone on diagnostic history. The
  historical execution retry API must not be represented as current coverage.
- status-busy-execution-v1 includes the rejection-only retry implementation;
  among this selected file set, only the ObservedCommand re-export differs.
- observation-blocked-descendant-v1 and delayed-teardown-v1 match all reviewed
  files. They directly exercise new completion/cutoff ownership, but neither is
  a replacement for all older preparation and successful cleanup cases.
- M4-T1 still requires the integrated admission/descriptor/gate and evidence
  scope review. The latest cutoff regression fixes the newly found API issue;
  this review does not silently award a full M4 pass from its passing tests.

## Remaining sections for this attempt

- Environment assumptions, command capabilities/seccomp, namespace creation,
  descriptor/proc boundaries, root confinement, runtime mount targets, admission
  and shell/cleanup protocols (including the complete M4-T1 reconciliation).
- Workspace/cache/verity/scute chain, loop/device-mapper ownership, filesystem
  mount/unmount/idmapping, pinned recovery anchors and serialized records.
- Network preparation/cleanup and resolver authority as coupled M4 requirements;
  retain explicit M6/M7 dual-stack and inbound-traffic qualification gaps.
- Full M4 evidence sufficiency, historical finding dispositions, all errors and
  API consumers across those sections; preservation and external-asset limits.

M5/M6 and platform/distro/CI/retirement gates remain separate obligations. When
all sections are reviewed, record the full result; findings reset the streak.
Do not count this segment, or a second segment, as separate clean passes.
