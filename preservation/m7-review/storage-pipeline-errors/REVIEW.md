# Typed storage errors at the pipeline boundary

Baseline: 5bf27da. Continued partial progress on errors-api-002/003.

Bootstrap, carapace and firmware coordinators now use concrete From conversions
instead of their generic Display-to-string error adapters for storage operations.
Mapping, snapshot, chain and volume-unmount failures preserve FailureDiagnostics,
including concrete original/cleanup I/O payloads and errno history. Volume mount,
loop attach and loop detach preserve the owned io::Error. Sealing preserves the
ChainError variant. Firmware capture/finalization preserves ExtractionError;
combined firmware extraction/cleanup failures retain both independent branches.
Volume discard also uses the typed snapshot-close and loop-detach conversions.

All conversions are methods in pipeline/failures.rs, without new trait machinery
or mutable state. The borrowing lower-level error owners cannot escape the
workspace. These terminal conversions preserve diagnostics then drop the owners,
as the previous string conversion did. The loop-detach conversion explicitly
documents that dropping/autoclear is not confirmed detachment. No cleanup retry,
success receipt, deadline extension, mount transition, or publication permission
is introduced. Recovery APIs retain their existing consuming owner contracts.

Source review: close-before-detach ordering, reverse chain cleanup ordering,
formatter cleanup, paired volume discard and firmware cleanup evaluation order
are unchanged. From conversions do not retry cleanup. Firmware's dual-failure
branch now uses OperationAndCleanup; no failed extraction can become output.
Only bootstrap's static setup diagnostics and generic stage rejection still use
the string adapter. Those are outstanding parts of the original review findings.

Validation (serial; CARGO_BUILD_JOBS=2,
TMPDIR=/var/tmp/conglobate-rewrite-tmp, CARGO_TARGET_DIR=target/m7-stable):

- Initial library run: 139 passed; retained separately.
- Final cargo +stable test --lib: 141 passed. New tests exercise a real chain
  failure conversion without kernel owners and combined firmware/storage error
  inspection, checking original errno and cleanup history remain separate.
- cargo +stable clippy --all-targets -- -D warnings: passed.
- git diff --check -- src: passed.

The VM result is recorded separately in storage-pipeline-bootstrap-v1/. Unit
coverage does not prove every storage failure branch; native bootstrap success
does not prove firmware replacement or error cleanup. No complete adherence
pass is claimed. errors-api-002/003 remain open; M4 remains 0/3.

Completed additional checks: lifecycle harness compilation passed. Native amd64
KVM bootstrap runner exited zero and the maintained bootstrap verifier passed all
six positive assertions (with target boot/full sandbox qualification explicitly
false). All 169 compared source/fixture files match the archived run. Source,
serial output, artifact and verdict are preserved with hashes for locally held
large VM assets. No durable remote archival claim is made.
