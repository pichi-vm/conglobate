# Pipeline error boundary audit

Baseline: 234a7dd. Focused closure of original errors-api-002, not a full
milestone adherence pass. Source hashes accompany this report.

## Original finding and contract

initial-critics.json identifies BuildError::Execution(String) and private string
adapters in the three coordinators. It explicitly downgrades the finding because
main only prints failures and returns failure. No new exit-code taxonomy is
required. The later action ledger asks for owned diagnostics preserving primary
and cleanup failures. InventoryError is separately tracked as errors-api-003.

## Source inspection

- BuildError has no Execution(String) or generic error-to-string constructor.
  Bootstrap, carapace and firmware use typed From conversions. Static invalid
  invocation messages are distinct from operating-system or lifecycle failures.
- FailureDiagnostics retains shared concrete io::Error payloads, exposes the
  primary and ordered cleanup history through read-only accessors, and follows
  the primary through Error::source. Consuming retry transitions extend history.
- OperationAndCleanup retains two BuildError values. StoragePairCleanup retains
  both tool and output failures. Standard source traversal follows the primary;
  public fields provide independent secondary causes. Display emits both.
- Command rejection retains the execution outcome, optional cleanup history and
  directive index. Successful retry does not erase previous cleanup failures or
  authorize acceptance of failed execution. Source traversal reaches command
  setup and observation I/O causes.
- Preparation, COPY and network rejection use final returned histories after
  cleanup attempts. Network cleanup precedes command/storage release.
- Storage conversions intentionally relinquish retry owners at the terminal
  pipeline boundary, after capturing diagnostics. This is documented in
  failures.rs, BuildError, Pipeline::run and into_terminal_parts. It is not a
  cleanup receipt or permission to reuse the VM. SANDBOX-DESIGN.txt requires
  disposal when teardown cannot complete. No new publication path is introduced.
- Formatter rejection attempts storage and formatter cleanup independently and
  combines failures. Firmware extraction combines extraction and cleanup errors.
- Pipeline publication follows successful stage completion and context.close;
  main prints the result and exits without attempting another build.

## Evidence and limits

The 150 library tests and strict all-target Clippy recorded in ../inventory-errors/
apply to this unchanged source. Relevant maintained tests inspect command errno,
exit status, standard source traversal, recovered retry histories, nested
formatter/runtime/tool/output failures, firmware extraction plus cleanup, and
ordered shared diagnostics. Earlier network-cleanup-history-v2 VM evidence covers
network rollback and retained expiry errors; command-error-missing-label-v2 covers
negative command setup/publication. These do not prove all lifecycle scenarios.

Verdict: original errors-api-002 is addressed. There is no reason to leave this
specific finding open for an undefined broader taxonomy audit. This verdict does
not close security/lifecycle qualification or constitute a full adherence pass.

## Separate finding: carapace directive position

OPEN: DerivedCarapace::derive iterates each directive and passes a one-element
slice to directives_with_network. That method enumerates from zero, so any
StageFailure from a later carapace directive carries index zero. Bootstrap and
firmware pass whole slices and do not have this defect. This is an existing
misleading diagnostic, not erasure of a typed failure. Correct it by carrying the
stage-relative position through rejection without discarding cleanup ownership
or nesting contradictory directive indices. Add regression coverage for a later
carapace directive. Until fixed, complete adherence remains NEEDS WORK.
