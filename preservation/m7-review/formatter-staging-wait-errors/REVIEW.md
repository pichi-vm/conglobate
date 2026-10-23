# Formatter staging and wait error preservation

Baseline: c814ab4. Continued partial progress on errors-api-002/003.

FormatterStageFailure::cleanup_with_source is a crate-private consuming method
that returns the original io::Error and the result of one cleanup attempt.
Existing public cleanup delegates and retains its original signature/behavior.
The pipeline now preserves FormatterStaging(io::Error), combining a failed
cleanup as the existing typed FormatterCleanup branch. No string round trip or
additional cleanup attempt is used. Source ownership transfers directly; the
backing remains owned by FormatterCleanupFailure if cleanup fails.

After formatter cancellation fails, the pipeline now preserves the existing
FailureDiagnostics in FormatterWait. Cloning shares immutable concrete io::Error
payloads, including original error and ordered cancellation errors. Previously
this same boundary formatted the failure and dropped it. The failure is still
dropped here: this change does not introduce retry authority into the terminal
pipeline API. Formatting retains its ManuallyDrop inputs when the process cannot
be reaped; VM disposal is still required. No cancellation, deadline, process,
mount, or publication behavior changed.

Validation, serial (CARGO_BUILD_JOBS=2,
TMPDIR=/var/tmp/conglobate-rewrite-tmp, CARGO_TARGET_DIR=target/m7-stable):

- cargo +stable test --lib: 139 passed.
- New staging tests cover failure before backing exists and an actual local
  directory cleanup failure. The latter preserves primary errno, returns a
  cleanup owner, restores the fixture, and successfully retries removal through
  that owner. This is ordinary temporary-directory I/O, without mounts or devices.
- New wait-diagnostic test verifies shared concrete payload and cancellation errno
  history. It does not create or leave behind an unreaped child.
- cargo +stable clippy --all-targets -- -D warnings: passed.
- cargo +stable check --manifest-path tests/lifecycle/Cargo.toml: passed.
- git diff --check -- src: passed.

No new VM run or full lifecycle qualification. Full stage/storage conversion and
owned diagnostic taxonomy remain open, so neither historical finding is closed.
This focused source/style/recovery review is not a complete adherence pass.
M4 remains 0/3.
