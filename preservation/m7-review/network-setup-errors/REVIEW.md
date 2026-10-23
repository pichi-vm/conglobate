# Network setup diagnostics and pipeline handoff

Baseline: 4805e03. Partial progress on errors-api-002/003.

NetworkStageFailure now exposes its immutable diagnostic history within the crate.
After network cleanup succeeds, it passes that history as a concrete io::Error
payload to command teardown instead of formatting it away. The pipeline uses the
same typed command-rejection method as other commands. If network cleanup fails,
the pipeline returns NetworkSetup(history) with its directive index; command
teardown is not attempted while a network creator may still be owned.

Initial NetworkSetupFailure construction now accepts Into<FailureDiagnostics>.
Early I/O and namespace input/launch/capture failures retain errno. Namespace reap
preserves WaitError as an error payload; creator exit preserves ExitStatus in an
internal typed error. The creator owner and pending variant are transferred
exactly as before. The internal exit wrapper is not a public inspection API;
complete public diagnostic taxonomy remains part of the outstanding audit.

No network teardown branch, owner, deadline or admission condition changed.
Link/configuration/NAT and cleanup conversions still explicitly wrap strings;
this change does not claim those branches now preserve typed causes. The expired
pending-owner behavior is unchanged. The former pipeline Display-to-string helper
is unused and removed; bootstrap's three static setup diagnostics now use
InvalidExecution(&'static str), eliminating Execution(String) from BuildError.

Validation, serial with CARGO_BUILD_JOBS=2,
TMPDIR=/var/tmp/conglobate-rewrite-tmp and CARGO_TARGET_DIR=target/m7-stable:

- Initial library run: 143 passed, with dead-code warning for the now-unused
  pipeline helper. Log preserved; helper removed before final checks.
- Final cargo +stable test --lib: 145 passed. New tests preserve early namespace
  errno and creator nonzero/signal status without acquiring privileged resources.
- cargo +stable clippy --all-targets -- -D warnings: passed.
- Maintained network-composite disposable VM case: passed, including production
  allocation contention/release, distinct live prefixes, rollback and expired
  teardown retry retaining its owner. Runner exited zero and VM powered off.
- All 156 compared source/fixture files match the VM snapshot. Run, assessment,
  serial/status/source and external large-asset hashes are in
  preservation/m7-review/network-error-composite-v1/.
- git diff --check -- src: passed.

No outbound/DNS qualification or pipeline network-failure injection is claimed.
Further work remains on concrete underlying network cleanup errors and the final
public taxonomy/ownership audit. Neither historical error finding is closed.
This is not a complete adherence pass; M4 remains 0/3.
