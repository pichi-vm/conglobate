# Architecture adherence: dedicated build VM revision

The user's clarification supersedes the former persistent-host guardian design.
Prior M2 passes are preserved but do not approve this changed lifecycle contract.
Review scope: DESIGN.md section 9/12/13; SANDBOX-DESIGN.txt trust model, policy
lifetime and failure/qualification sections; ARCHITECTURE.md ownership and
lifecycle; REIMPLEMENTATION.md milestone criteria. Rust ownership/style invariants
and pipeline/input/output scope remain requirements. No code qualification is
implied by an architecture pass.

## Review with findings — clean streak 0

Inspected the active documents for residual guardian, recovery, crash, anchor and
concurrent-build obligations. Found stale section 12.3 requiring parent-death
recovery/concurrent builds despite dedicated-VM disposal. Corrected it and clarified
DESIGN.md's interrupted activation cleanup. Preserved immutable authorization and
identity lifetimes through the shutdown interval, independently enforced disposal,
failed-build rejection, and in-process fallible teardown. Deferred component
retirement; preserved previous documents and all source/evidence. Three consecutive
clean reviews of the corrected contract are still required before M4 resumes.

## Clean self-review 1 — requirements first

Traced the revised DESIGN and SANDBOX requirements to architecture ownership and
milestone acceptance. Dedicated VM disposal replaces only abrupt-process recovery;
normal timeouts, descendant quiescence, fail-closed publication, and isolation
remain. Reviewed safe Rust, sealed states, consuming transitions and fallible
RAII completion requirements: unchanged. No new implementation or qualification
claim. Previous documents and source remain preserved. No findings; streak 1.

## Clean self-review 2 — ownership and evidence first

Read the lifecycle, teardown traceability and milestone review categories against
revised requirements. Policy and identity lifetimes remain coupled through the
shutdown interval; destructors are not completion evidence. Typed ownership and
error retention remain required. Actual environment shutdown and integrated
sandbox qualification remain outstanding. Verified no production-source diff.
No scope or preservation regression. No findings; streak 2.

## Clean self-review 3 — preservation and failure paths first

Verified all four archived document SHA256 values. Re-read the complete design
and sandbox diffs, architecture state/error ownership, qualification table and
milestone criteria. No recovery service is required, but failed cleanup cannot
advance or publish. Safe-library and immutable/consuming Rust style requirements
remain intact. Historical component tests are explicitly insufficient to qualify
the executor. Full pipeline and platform scope remains outstanding. No findings;
streak 3. These are self-reviews of architecture, not independent code reviews.

Frozen reviewed document hashes:

- DESIGN.md: 25b3bd8fe02b337aef0de188d5b5ccdf9a5f7066cfc1e75865c8b191563ac16b
- SANDBOX-DESIGN.txt: b94bf3ab6e6491b17536b11a6194cf4a428c88e5243849710a3f71e84cd69e0d
- ARCHITECTURE.md: 68c63f11c31d6f3b840967dcf35c3ef556b377166643f9bbd0c04b02e7588aee

Ledger-only updates record acceptance of this revised M2 contract. M4 resumes;
its implementation has no clean milestone adherence passes yet.
