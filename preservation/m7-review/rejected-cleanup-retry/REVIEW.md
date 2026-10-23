# M4 recovery acceptance finding

Baseline: 1cafa57. This is an interrupted adherence review with a finding,
not a completed clean pass. M4 streak remains zero.

SANDBOX-DESIGN 10.3–10.5 requires cleanup errors to reject the build and
preserve the original outcome separately from cleanup diagnostics. Reading
src/step/execution.rs revealed retry_cleanup called after_finalization/accept:
a successful command plus a transient cleanup failure could become an
ExecutedStorage receipt after successful cleanup retry. Pipeline reject methods
already discarded that receipt; no actual publication bypass is demonstrated.
The public library contract nevertheless allowed forbidden acceptance.

Correction: retry_cleanup consumes and returns StorageExecutionFailure. It
can transition only among teardown, finalization and completed rejected storage.
Only the initial lifecycle can call accept. Completed recovery preserves original
cleanup diagnostics; immutable histories clone their Arc error payloads. No
new runtime acceptance flag, public receipt constructor or unsafe operation.
Pipeline rejection now recovers only rejected storage for disposal.

Evidence: full rootless suite, 51 documentation tests, strict root and maintained
lifecycle-fixture Clippy. New tests explicitly begin with successful command
status and failing finalization, reject accepted-receipt construction, and check
diagnostics across failed then successful retry. The compile-fail regression
would accept the old retry API and rejects the corrected API. Test storage is
a private sealed in-crate fixture; no production seam or privileged host test.

This does not constitute whole-M4 source review, fresh VM execution of the
changed high-level recovery API, or M5–M8 qualification. Existing VM regression
evidence remains scoped to its recorded sources. Full M4 review must restart
on the corrected tree; the prior test commits were concrete progress.
