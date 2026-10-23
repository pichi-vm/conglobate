# Terminal runtime error preservation

Baseline: 6a34d34. Partial progress on errors-api-002 and errors-api-003.

The pipeline previously converted runtime creation, cleanup, and combined
operation/cleanup failures into Execution(String). A failed cleanup therefore
erased an otherwise typed operation error. The pipeline now distinguishes
RuntimeCreate, RuntimeCleanup, and OperationAndCleanup. Runtime errors retain
the underlying io::Error (including errno, kind and source). Combined failures
retain the operation variant and cleanup variant independently; Error::source
follows the primary error, with the secondary cause available through its field.
Stage rejection also uses the combined variant instead of concatenating the
primary diagnostic with the cleanup diagnostic.

Resource contract: RuntimeFailure still owns retry authority until consumed.
Its crate-private into_terminal_error is used only by the pipeline conversion.
It drops retained runtime owners exactly as the previous string conversion did;
RuntimeMounts::drop logs retained mounts and does not unmount them. This boundary
requires dedicated VM disposal and provides no cleanup receipt or build retry
permission. No cleanup attempt, deadline, acquisition or unmount order changed.
The design intentionally does not manufacture a 'static recovery owner from a
failure borrowing the workspace.

Validation, serial with CARGO_BUILD_JOBS=2,
TMPDIR=/var/tmp/conglobate-rewrite-tmp and CARGO_TARGET_DIR=target/m7-stable:

- cargo +stable test --lib: 134 passed. Three new tests cover runtime create vs
  cleanup classification, exact errno retention, preservation of both typed
  causes and both display messages, and unwrapped operation-only failure.
- cargo +stable clippy --all-targets -- -D warnings: passed.
- cargo +stable check --manifest-path tests/lifecycle/Cargo.toml: passed.
- git diff --check -- src: passed.

Only explanatory doc comments were added after these checks. No new VM run;
unit tests use resource-free failures and do not prove retained-mount cleanup.
Earlier runtime-scope VM evidence remains historical, not a current-source run.

Remaining: stage primary failures, formatter diagnostics, storage failures, and
paired storage discard can still flatten errors into strings. A complete owned
diagnostic taxonomy, failure ownership disposition and CLI exit taxonomy remain
open. Do not mark either finding resolved. This is not a complete adherence
pass; M4 remains 0/3.
