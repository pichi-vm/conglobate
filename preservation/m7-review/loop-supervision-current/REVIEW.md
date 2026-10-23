# Loop and supervision test review

Base c5a6b4e. Production behavior unchanged.

- tests-005 (addressed after reconsideration): The private LoopDevice::attach validates length before opening loop control, so a unit test can exercise rejection safely. Added zero and misaligned lengths for both access markers, asserting InvalidInput and the specific diagnostic; no valid backing or privileged device access requested. Positive attach/detach remains VM-only.
- tests-010 (partially accepted; launch deadline widened): Already-paired hold tests have no legitimate successful completion and keep their short negative waits. launch_supervised also performs startup under the deadline, so the prior blanket rejection missed startup sensitivity. Increased only that test to one second, with the same Started/TimedOut and cleanup assertions. This reduces ordinary CI sensitivity, not a proof against arbitrary scheduler stalls.
- ownership-002 (duplicate; current fix verified): Same two-buffer matcher as performance-010: both vectors allocated before token loop, cleared/swapped per iteration. Current source inspected; no per-token allocation remains.
- errors-api-004 (duplicate; current fix verified): Same copy-mode public-surface fix as simplification-020. Current copy module exports only CopyMode and ModeError; parser representation remains private.
- performance-007 (duplicate; current policy disposition retained): Same unannotated decoded-size resource-policy issue as security-001/002. Current branch still measures to EOF with authenticated reads; no absolute decoded-size cap introduced. Environment resource policy remains distinct; no exhaustion-resistance claim.

Validation: loop rejection unit test and all seven supervision integration tests passed; strict all-target Clippy passed. Initial format check requested wrapping the changed launch expression; cargo fmt applied that formatting. Tests use stable Cargo --locked with CARGO_BUILD_JOBS=2, TMPDIR=/var/tmp/conglobate-rewrite-tmp, CARGO_TARGET_DIR=target/m7-stable. No synchronization sleep, new privileged execution or full milestone pass. Prior raw VM and source evidence retained.
