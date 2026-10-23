# Parent full-architecture review: preliminary findings

Frozen baseline fc0ad91. No source or design edits while Claude reviews this scope.
This review has findings and cannot count as a clean M2 adherence pass.

## Substantive ownership correction required

ARCHITECTURE Error and recovery ownership says Step<Quiescent> retains the
observed command outcome and teardown produces that outcome. Source contradicts
that description: StepState<Quiescent>::Resources is DrainedHelper; Finishing
holds the trusted helper exit status and ReapedChildren. CommandOutcome is held
separately by CommandResult in step/outcome.rs and carried into ExecutionOutcome
and StorageExecutionFailure in step/execution.rs. FinalizedStorage explicitly
warns that its status is the helper status, not command outcome. Execution
acceptance requires outcome.success() after finalization. Correct architecture
to describe this separation; do not modify sound source to match the stale prose.
This matters because successful cleanup or helper exit cannot certify recipe
success. Outcome/cleanup unit tests cover that distinction.

## Test-environment wording to reconcile

DESIGN 1.1 says primary integration tests run without VM infrastructure while
DESIGN 9 and SANDBOX require disposable-VM qualification for privileged work.
Rootless input/plan/pure transform tests can run directly; privileged lifecycle
checks must remain in disposable guests. Clarify test categories without coupling
the binary to a VM implementation or weakening the dedicated-VM deployment.

## Other inspected categories

- Local commit/path/cache lock contract, stage combinations and all-component
  inheritance, variable rules and transform defaults/types: architecture delegates
  exact schema to normative design and current test anchors are explicit.
- Formatter profile encodes typed capacity/label and fixed args/config identity;
  source separates immutable invocation description from executed receipts.
- Volume sealed state resources and borrowing prevent extracting mounted snapshot
  or unmounting while a view is borrowed; failure owners retain authority.
- CompletedImage is private, constructed after inventory output validation;
  publication cannot be inferred from inventory decoding alone.
- Current minimal VM evidence and full distro/platform/legacy-fault gaps remain
  explicitly distinct. Historical scoped probes are not current qualification.
- No historical artifacts removed. Durable external-asset recovery is still M8
  work; no claim that temporary paths alone meet it.

Await terminal Claude review for further source-grounded reconciliation. These
observations are architecture review, not M4-M8 acceptance or a clean pass.
