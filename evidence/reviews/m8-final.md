# M8 retirement adherence

Scope: production source unchanged since 5d1f358; maintained tests, CI,
documentation and archival retirement. This is a focused completion review,
not a renewed general sandbox audit. M1–M7 evidence and original finding
resolutions are reused.

Before the clean streak, review found an obsolete confidential-hardware gate,
a stale dual-root validation statement and outdated harness instructions.
Corrected them against the user's deferral, pipeline evidence and current
runner. ORAS on the Ubuntu host also hit user-namespace policy denial; moved
the same assertion into the disposable guest without changing host policy.

## Pass 1 — scope, source and style

Compared the final workflow with the agreed environment/binary separation and
ARCHITECTURE.md. No production source changes, pichi binary dependencies,
network allocation, policy registrations or new mutable workflow machinery.
The ORAS fixture uses an owning method, safe Rust, the real Pipeline and a
committed recipe. It compares the exact manifest and complete blob set/bytes;
it does not claim OCI copying attests the separate inventory. Native Arm64
Rust and lifecycle jobs remain; confidential hardware is explicitly deferred.
Clean.

## Pass 2 — lifecycle and failure evidence

Reviewed VM runner dispatch, guest guard, ORAS input staging and expected
marker/status/poweroff checks. The new fixture passes all three checks in
oci-vm/assessment.json; locked fixture compilation passes. CI pins the ORAS
archive digest and runs it only in the amd64 guest. Existing seven lifecycle
cases remain on both architectures. Host user-namespace denial is preserved
as a failed attempt, not skipped into a pass. Rust ownership/cleanup contracts
are unchanged and reuse the accepted M4–M7 reviews. Clean.

## Pass 3 — preservation and claims

Verified retirement prerequisites: 84,489 worktree entries and 140,118 external
entries have content/mode/symlink verification receipts; no baseline-present
external path disappeared. Git bundle preserves committed history; post-snapshot
work is retained by a final supplementary bundle. Original uncommitted review
streams were archived before removal. No external originals were deleted.
Active harness code has no references to removed trees. Current docs distinguish
historical source versions, native Arm runners versus guest TCG, representative
builds versus universal compatibility, and deferred confidential qualification.
Production safe Rust/typestate/RAII contracts remain unchanged. Clean.

Three consecutive clean self-review passes. Final CI is a separate completion
condition: both workflows must pass at the published retirement commit. Receipts
are retained in the archive's final-ci directory so recording success does not
require an endless sequence of documentation-only CI runs.
