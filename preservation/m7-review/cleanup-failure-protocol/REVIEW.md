# Cleanup failure handoff: resolver-cleanup-diagnostic-001 resolved

The trusted receiver now reports local resolver/mount/scope cleanup failure before
releasing the admitted channel. It waits for parent loss under a fixed deadline;
there is no sleep, retry extension or command authority change. The parent reads
the failure as an error, then follows existing cancellation/drain/rejection logic.
A successful shell exit remains distinct from successful receiver cleanup.

Private protocol opcode 12 carries canonical CBOR [1,12,text], no descriptors,
maximum 1024 UTF-8 bytes. Sending rejects empty/oversized values; the reporting
method truncates oversized diagnostics at a character boundary. Receiving checks
canonical encoding, payload bounds, invalid UTF-8, ancillary descriptor count
and trailing bytes. Existing fixed-size opcodes retain their validation. The
process-boundary error is diagnostic text in io::Error, not a reconstructed Rust
error or errno. Cleanup success remains a different opcode and receipt.

Tests: 146 library tests and strict all-target Clippy pass. New unit tests cover
bounded/canonical messages, invalid payloads and descriptors, and queued error
delivery after helper exit while shell success remains independently readable.
Initial compile failures (moving a borrowed packet and capturing a borrowed
namespace owner too early) are preserved. Fixes borrow packet for encoding and
sequence namespace restoration after consuming the view; no cloning authority.

Native amd64 negative VM passes the strengthened verifier: the supervisor's own
error line contains the resolver identity-mismatch cause, build rejected, no
published output, external network retained. It does not claim cleanup completed.
Positive VM passes dangling scaffolding, command-time parent rename, preserved
author replacement, DNS/TCP and successful publication. Both source comparisons
differ only in the verifier assertion tightened after snapshot creation; all
application source matches. Verifiers were run against their actual raw results.

Earlier failed VM evidence remains under resolver-lifecycle/. New raw logs,
source/context archives and artifact metadata are retained here. Large assets are
hashed locally, not remotely durable. This targeted finding closure is not a full
M4 adherence pass or platform qualification; M4 remains 0/3.
