Both commits are clean and implement exactly what was agreed. Here is the reconciliation.

## The clone probe closes tests-001 — with one precise scope limit

**Why the mechanism works where my `unshare` proposal failed.** `unshare` is not in `ordinary()` at all, so the old marker tested the default-deny path and never reached the `__NR_clone` rule. The C probe calls `clone(2)` directly — glibc's `clone()` wrapper issues legacy `__NR_clone`, not `clone3` — so it lands exactly on the `MaskedEq(0x7e020080)` condition. That is the rule under test.

**The capability confound is genuinely eliminated**, which was the hard part. Both arms `prctl(PR_SET_NO_NEW_PRIVS, 1, ...)` and both assert `CapEff` bit 21 (`CAP_SYS_ADMIN`) is still set, returning 4 otherwise. So the control arm proves the kernel *would* permit every namespace clone, and the filtered arm — same capabilities, same NNP, differing only by `CommandSyscalls::apply()` — proves EPERM is attributable to seccomp. That is a real positive control, not the vacuous one I proposed.

**Benignity confirmed by reading:** fixed compile-time flag constants (no argv-driven flags), gated on `/conglobate-disposable-validation-vm`, no `CLONE_VM`/`CLONE_FILES`/`CLONE_FS`/`CLONE_THREAD`, child is `return 0`, parent `waitpid`s with EINTR retry, stack freed. Application Rust stays `#![forbid(unsafe_code)]` — the unsafe is isolated in a separate C test fixture that never ships. No untrusted command gains capabilities; `clone-filter` is a trusted fixture mode running no image commands.

**Mask coverage, computed not assumed:**

```
mask              0x7e020080
seven probed OR   0x7e020000
unprobed remainder      0x80
```

The seven probed flags cover every bit of the mask **except `0x80`**. Your note is right and worth stating precisely: `CLONE_NEWTIME` is `0x80`, which collides with the **exit-signal byte** (`mask & 0xff == 0x80`), and the probe passes `flags | SIGCHLD` (`0x11`). So `0x80` cannot be exercised as a time-namespace control through this probe, and you correctly do not claim it. `MaskedEq` matches if *any* masked bit is set, so the rule is sound for the seven; the `0x80` term remains structurally unverified. That is an honest residual gap, not an overclaim.

**Verdict: tests-001 → fixed for the seven supported legacy namespace flags.** The `0x80`/`CLONE_NEWTIME` term stays an explicit, documented coverage gap. I retract my `ENOSYS` demand entirely — EPERM is the correct production behavior and should not change for a test.

## Runner delta — reviewed explicitly, not asserted as byte-identical

I diffed the captured archive against the current runner rather than claiming equality. `clone-probe.c` and `src/main.rs` are byte-identical; the in-VM `fixture.sh` script is identical. The **only** difference is three subset-mode verdict lists (lines 374 pipeline, 377 network, 382 formatter-failures) that wrongly required `NATIVE_CLONE_NAMESPACE_MASK_PASS` — a marker those subsets never emit, since only `clone-filter` prints it. The archived runner has it in 4 places; the current runner in 1 (line 396, full mode). **Full-mode behavior under test is unchanged**, so the in-flight v262 full run is unaffected by the fix. The archived-vs-current distinction is recorded, not glossed.

**v262 status: no verdict.json exists yet.** I claim no full-suite pass. What I verified is narrower and true: serial.txt records all 16 cases (8 control PASS errno=0; filtered `ordinary` PASS errno=0 plus 7 PASS errno=1/EPERM) and `NATIVE_CLONE_NAMESPACE_MASK_PASS`.

## Historical markers

Agreed — retain them. `NATIVE_SYSCALL_NAMESPACE_DENIAL_PASS` says *namespace denial*, which is what it tests, and it is still meaningful as default-deny coverage for `unshare`. **I withdraw my "misleading coverage" finding from round three**; the name does not claim clone-mask coverage and I over-read it. The real gap was never-tested mask logic, which the probe now closes.

## Dispositions for 5460f68 and 0496311

- **errors-api-005 → fixed.** `TryFrom<u64> for FilesystemSize` added; `FromStr` delegates to it, so bounds (16 MiB–64 GiB, 4096-multiple, non-zero) are shared, and `src/inventory.rs:215` now validates the decoded `u64` directly instead of the `format!("{}B", …).parse()` round-trip. The misleading `ValueError::Syntax` path on numeric input is gone. `tests/values.rs` gained coverage.
- **simplification-002 (sub-finding) → fixed.** `src/network/namespace.rs:96` now collects `Result<Vec<_>, _>` and propagates, so per-entry `read_dir` errors are no longer silently counted as tasks. This was the one correctness-flavoured part of that finding; the remaining 9-site diagnostic-helper consolidation stays **accept**.
- **errors-api-006 → fixed.** Manual `Display` writes `{outcome}; cleanup: ` then either the `FailureDiagnostics` Display or `"complete"`. Retained ownership is untouched (`cleanup_error()` only borrows) and no `source()` chain was added — exactly the shape we agreed. Note `thiserror::Error` was replaced with a hand-written `impl std::error::Error`, which is correct here since the derive cannot express the conditional arm.

## Updated counts

**fixed 28 · accept 30 · reject 40 · open 9 · duplicate 5 = 111**

Movement this round: tests-001, errors-api-005, errors-api-006 → fixed (3 from accept); simplification-002 partially satisfied but stays accept for the remaining consolidation.

## Remaining disagreement

**None on mechanism.** My round-three `unshare` control is withdrawn as unworkable; my `ENOSYS` demand is withdrawn as production-distorting; my "misleading marker" finding is withdrawn as an over-read.

Two scope notes I'd keep on the record, neither a disagreement:
1. `CLONE_NEWTIME` (`0x80`) is unverifiable through this probe because it aliases the exit-signal byte — document as a residual gap.
2. v262 lacks a verdict; the clone evidence stands on its own markers, and no full-suite or qualification claim follows from it.

`policy-alternative.md` remains discussion only. M7 at zero. No edits, no host execution of the probe, no privileged operations.
