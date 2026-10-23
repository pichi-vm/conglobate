# Formatter and paired cleanup diagnostics

Baseline: 26a1be0. Partial progress on errors-api-002/003.

Formatter launch and completion already return an io::Error, and successful
cancellation returns FailureDiagnostics. reject_format now accepts conversion
into FailureDiagnostics instead of Display, retaining original errno and ordered
failed-cancellation history. FormatterCleanup retains the concrete owned
FormatterCleanupFailure rather than its string. The ordinary post-format close
uses this variant too. The formatter backing retry owner therefore remains in
the terminal error until that error is dropped, instead of being dropped during
formatting of its message. This does not authorize continuing the build.

OperationAndCleanup composes the primary error with storage failure and then
formatter-runtime failure. Both cleanup operations are evaluated first, as
before. Snapshot close must still succeed before detachment; failed close does
not authorize detachment. Paired storage discard still attempts both volumes
before matching outcomes. Its dual-error case now preserves named tool and
output branches in StoragePairCleanup instead of concatenating strings.

BuildError::with_cleanup consumes the operation and an already-completed cleanup
result. It does not perform cleanup or provide a success receipt. All transitions
remain consuming; no runtime flags, unsafe code, synchronization or deadlines
were added. Standard Error::source follows the primary branch; secondary causes
remain inspectable through enum fields.

Validation (serial; CARGO_BUILD_JOBS=2,
TMPDIR=/var/tmp/conglobate-rewrite-tmp, CARGO_TARGET_DIR=target/m7-stable):

- Initial library check before new tests: 134 passed; log preserved.
- cargo +stable test --lib: 136 passed. New tests retain the formatter's original
  and cleanup errno history plus independent tool/output and subsequent runtime
  errors, inspect every branch, and check display includes every cause. Successful
  cleanup preserves the original formatter variant without wrapping it.
- cargo +stable clippy --all-targets -- -D warnings: passed.
- cargo +stable check --manifest-path tests/lifecycle/Cargo.toml: passed.
- git diff --check -- src: passed.

Tests exercise diagnostic representation without kernel resources. They do not
inject actual formatter or storage cleanup failures. No new VM qualification.
The error text gains explicit formatter-execution context and generic cleanup
composition; callers should inspect variants rather than parse strings.

Remaining: formatter staging and unreaped formatter wait failures, generic
storage conversion, and stage primary errors still erase typed details. Complete
failure taxonomy/ownership disposition remains open. Neither review finding is
resolved; this is not a complete adherence pass. M4 remains 0/3.
