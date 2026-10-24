# Binary simplification

Baseline: 8c2d294, clean tree; 18,587 production Rust code lines (tokei),
20,789 physical lines excluding trailing unit-test modules and firmware/tests.rs.
The baseline remains in Git and the prior verified archives.

Contract: standalone binary in a disposable VM. Preserve safe Rust, resource
ownership, useful typestate, isolated commands, finite deadlines, quiescence,
writeback, local verified inputs and atomic publication. Cleanup failure is
terminal and requires VM disposal; no build or storage recovery API is needed.
Use io::Result and propagation for I/O; anyhow at orchestration boundaries.
Context should identify a stage, command, image or path, not rename every call.

S1 baseline/plan: complete, three clean source reviews: (1) source concentration
and callers identify lifecycle/recovery as main target, (2) failure semantics
retain ownership until quiescence/disposal, (3) test migration preserves actual
assertions and records lost doctest coverage explicitly. No code changes yet.
S2 private binary/modules and test-only fixture: pending.
S3 error propagation and terminal cleanup: pending.
S4 lifecycle/assigned-volume consolidation: pending.
S5 docs, focused regressions, native architecture CI and measured reduction: pending.

Target about half the production code if justified by actual simplification.
Do not count moving code into another crate or deleting required checks as a
reduction. Three consecutive clean adherence reviews gate each milestone.

S2 implementation: removed library target; private modules shared only by the
normal binary and explicitly enabled VM fixture binary. Existing contract tests
are now binary-local unit tests (230 pass). Fixture compiles. Process-spawn test
selectors were updated for their new module-qualified names. Initial local test
run hit existing /tmp quota and stale selectors; rerun used owned storage TMPDIR.
Old compile-fail doctests are no longer executed by rustdoc (binary-only crate);
CI now compiles the VM fixture instead. No claim that this replaces negative
compiler assertions. Runtime invariants/tests remain and typestate still compiles.

S2 clean reviews: (1) Cargo targets and namespace visibility expose no library,
fixture is opt-in; (2) helper re-exec and guest guard unchanged, actual assertions
retained, selectors correct; (3) locked dependencies, 230 unit results and fixture
compilation verified, test packaging only. 3/3. Compiler unused-API warnings now
expose the removal candidates for S3/S4; final CI must be warning-free.
