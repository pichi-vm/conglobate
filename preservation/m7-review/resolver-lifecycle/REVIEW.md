# Resolver lifecycle command-time validation

Positive VM run passes: absolute dangling symlink with multiple missing ancestors,
read-only injection, actual DNS/TCP, subsequent scaffold removal, preservation of
the symlink, command-time parent rename/replacement and cleanup via the pinned old
parent while author replacement bytes survive. Export and independent exit pass.

Negative v1 reaches identity mismatch but the fixture service expects an exchange
not made by the recipe. Its timeout prevents final environment/output checks. Raw
failed evidence retained. v2 adds the exchange; output absence and retained external
network pass, but the strict diagnostic assertion fails: only joining peer closed
reaches supervisor diagnostics. v1 observed the specific error, v2 did not. Neither
negative run is represented as passing the full verifier.

New M4 finding resolver-cleanup-diagnostic-001: execution/helper.rs uses ? on
resolver.release before returning to the top-level error reporter. The admitted
channel drops during error propagation. Supervisor cleanup_until observes EOF;
AwaitingCleanup transitions to Running and cancellation can kill the helper before
its outer error reporter writes. Thus the specific cleanup cause is not reliable.
Rejection remains fail-closed; successful shell exit is not accepted as cleanup.
A bounded explicit cleanup-failure message on the trusted protocol is the next
fix to evaluate. No sleeps, delayed cancellation, or weaker verifier is warranted.

Production Rust was unchanged in these runs. Source comparisons distinguish the
later negative recipe/verifier changes from tested application bytes. Maintained
recipes/verifier and documentation are added; Python syntax and whitespace checks
pass. This regression remains failing until the protocol diagnosis is addressed.
No full M4 pass. Large assets locally hashed, not remotely durable. Raw failures,
source/context snapshots, assessments and positive artifact metadata retained.
