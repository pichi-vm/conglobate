# M3 adherence pass 3 — clean

Frozen scope 28a108b, self-review, evidence-to-contract order. Streak 3/3.

Requirements: checked tests against the agreed committed-input interface rather
than old checkout behavior. Both Git formats are exercised; capture survives
working-tree mutation and removal of the source object database. Recipe/lock
receipts hash the selected committed files, including confined symlink selection.
LFS/submodule negatives and duplicate ancestor-tree rejection are concrete tests.
Pass-through/transform-only plans ignore networking and identity execution inputs;
all five tested execution variants require maps, including no-run firmware and
COPY-only carapace. Index/manifest/content verification remains local and host-only.

Style/safety: verified that test fixtures use Repository/Store methods, validated
values are private and immutable, and no raw deserialization yields CapturedBuild
or ResolvedBuild authority. CapturedContext no longer owns a redundant materialized
checkout. Safe Rust and read-only interfaces remain enforced; no extra mutable
runtime state was added to accommodate input capture or the tests.

Failure paths: malformed source bytes cannot return a captured context; missing
pins/blobs and unsupported execution roots reject before mounted execution. No
network/download fallback. Optional networking is one captured aggregate, and
its file handles drop normally on failed input capture. Required maps are validated
values, not late unchecked numeric strings. Output publication remains a separate
later phase and cannot be reached from these input-only owners.

Evidence/preservation: 48 targeted tests plus strict all-target Clippy passed on
this scope; previous broader tests and VM evidence are retained with their source
identities. Tests are evidence, not the three review passes. No reference assets,
user files or failed experiments were removed. Remaining privileged/boot/platform
qualification is outside M3 and still outstanding. No findings. M3 gate is complete.
