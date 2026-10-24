# M3 adherence pass 1 — clean

Frozen scope 28a108b, self-review, request-to-plan order. Streak 1/3.

Requirements: explicit recipe/lock paths are context-relative; commit syntax is
full SHA-1/SHA-256 and repository object format must agree. ObjectReader uses
isolated Git settings, verifies every consumed object and captures selected tree
bytes/modes/links. No checkout/filter execution, fetch, LFS or submodule expansion.
Recipe raw forms preserve omitted/null/empty distinctions, reject unknown/duplicate
fields and validate from rules before BuildPlan. Input roles and stage order are
immutable; firmware tools do not become output ancestry. Required mappings cover
bootstrap, COPY-only/run carapace, explicit/implicit firmware; pass-through omits them.

Style/state: constructors convert raw inputs to private validated values; read
accessors expose immutable descriptions. Path marker traits are sealed. Context
borrows pin captured bytes rather than reopening Git. No public mutation or new
utility function architecture; fixture entrypoints are test-harness functions.
Safe Rust is enforced by crate lints. No privileged resources are acquired here.

Failures: missing/malformed documents or references fail before execution. Cache
resolution is local read-only and requires a compatible unambiguous host index.
Content verification is separate from path/digest syntax. Input capture returns
no plan after error, and a previous receipt never permits a later unchecked read.
No output is created by the new preflight regression.

Evidence: 48 focused input tests pass and strict all-target Clippy passes. The
new regression covers all execution entry variants with missing mappings and
independent UID/GID counts. Existing fixtures cover packed Git, confined links,
post-capture source mutation, missing blobs and invalid selection. These are
rootless input proofs, not command sandbox or boot qualification.

Preservation: historical tests/source/evidence and user-untracked files unchanged.
No finding after the added boundary coverage. No milestone beyond M3 advanced.
