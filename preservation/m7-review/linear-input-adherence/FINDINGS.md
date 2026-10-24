# M3 review preparation

Source reviewed at 83265df plus the architecture-gate ledger commit 742a8e5.
The committed-byte context implementation has object hashing, immutable captured
entries, confined symlink traversal, isolated repository configuration and explicit
LFS/submodule rejection. The current input tests exercised missing maps for a run
stage, but did not cover bootstrap, firmware extraction with no run, or COPY-only
carapace preflight. Added a rootless boundary regression for all these cases,
including implicit firmware tools and independent UID/GID counts. It uses a
missing cache/output and proves input capture does not acquire/create them.
This is a coverage finding, not a production behavior change. Streak remains zero
until three full reviews of the frozen corrected scope. Focused input suite and
strict all-target Clippy pass; logs retained. No privileged retest needed for a
rootless test-only change. No reference or user-untracked data removed.
