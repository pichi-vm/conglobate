# Command diagnostic source-chain audit

Baseline: 7062a5f. Focused continuation of errors-api-002/003 audit.

Finding: BuildError retained ExecutionOutcome and CommandOutcome, but their
thiserror definitions omitted source annotations. A caller traversing standard
Error::source stopped at the outcome rather than reaching setup/observation
io::Error. StorageExecutionFailure's manual Error impl also returned no source.
The first regression run reproduced this: expected errno 13, found no I/O cause.
That failed run is preserved in regression-before.log.

Added explicit sources for ExecutionOutcome::SetupFailed, ExecutionOutcome::Command
and CommandOutcome::ObservationFailed. StorageExecutionFailure::source now follows
the primary outcome. Its existing cleanup_error() API retains the independent
secondary history; a single Error::source chain cannot represent both branches.
ExitStatus remains a typed terminal outcome and has no artificial error cause.
No display text, resource, retry, deadline or command-admission behavior changed.

Regression test traverses only the standard Error interface, starting at an
indexed BuildError. It verifies errno 13 for both setup and observation failures.
After the fix all 147 library tests passed, strict all-target Clippy passed,
and the maintained lifecycle harness compiled. Commands ran serially with
CARGO_BUILD_JOBS=2, TMPDIR=/var/tmp/conglobate-rewrite-tmp and
CARGO_TARGET_DIR=target/m7-stable using cargo +stable.

No new VM validation was needed for source annotations. Existing VM evidence is
historical and not relabeled as a current-source run. This focused audit does
not establish the whole diagnostic/public ownership taxonomy or count as a
complete milestone adherence pass. Both historical findings remain open pending
that broader audit; M4 remains 0/3.
