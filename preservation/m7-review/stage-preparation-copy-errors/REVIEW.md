# Typed pre-step, preparation and COPY rejection

Baseline: 506256b. Partial progress on errors-api-002/003.

StageFailure::Before now retains the owned io::Error and directive index in
BuildError::Directive. Preparation and COPY retain their existing immutable
FailureDiagnostics in phase-specific variants, also wrapped with the directive
index. Failed rollback/retry conversions preserve their typed diagnostic history
instead of formatting it. The original error and a failed cleanup remain separate
OperationAndCleanup branches. No new generic trait or runtime state was added.

Source review covered all six rejection branches. Their cleanup sequence is
unchanged: pre-step discards storage; preparation rolls back then discards only
returned storage; COPY retries finalization then discards only rejected storage;
command and network paths retain their existing ordering. Returned rejection
storage is not a successful command receipt. All owners still move through the
same consuming transitions; no extra retry or deadline renewal is introduced.

The first Clippy run rejected three needless returns in the draft. That log is
preserved. The final code has one exhaustive match, no mutually delegating helper,
and no unreachable fallback. No production behavior was observed from the draft.

Validation (serial; CARGO_BUILD_JOBS=2,
TMPDIR=/var/tmp/conglobate-rewrite-tmp, CARGO_TARGET_DIR=target/m7-stable):

- cargo +stable test --lib: 141 passed.
- cargo +stable clippy --all-targets -- -D warnings: passed after correction.
- cargo +stable check --manifest-path tests/lifecycle/Cargo.toml: passed.
- git diff --check -- src: passed.

No new VM run and no new failure-injection coverage. Tests/compilation plus the
source comparison support this diagnostic refactor; they do not prove complete
stage cleanup. In particular, command/network primary outcomes and their cleanup
errors still use string conversions. Network setup itself also constructs some
string-only io::Error payloads. Successful recovery still follows the existing
policy of retaining the initial primary and discarding returned diagnostic
metadata when recovering storage; comprehensive history retention needs further
review. These limits remain open, not silently treated as resolved.

This is not a full adherence pass. M4 remains 0/3.
