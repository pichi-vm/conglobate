Verified independently at this head: 199 tests, 54 doctests, strict Clippy — all pass.

## Verification of eb767f3

**Typed bootstrap diagnostic — restored correctly.** `StorageLayout` gained `const LABEL: &'static str`, with `"step"` for `Volume`, `"bootstrap"` for `BootstrapVolumes`, and `NetworkedStorage` forwarding `R::LABEL`. The `#[error("{} storage finalization: {diagnostics}", R::LABEL)]` restores the exact pre-72293a1 bootstrap text. This is better than the manual `Display` I suggested — it keeps `thiserror` and makes the label part of the sealed layout contract. **My simplification-001 flaw is resolved.**

**`send_bootstrap_until` — removed.** The whole `impl Step<Ready, BootstrapVolumes>` block is gone and the now-unused `Instant` import dropped. `src/step/bootstrap.rs` is down to the typestate impls plus `with_bootstrap`. **simplification-006 → fixed.**

**COPY subtree ranges — correct, and I verified the subtle part empirically rather than by reading.** I compiled two standalone probes:

- `Path`/`PathBuf` `Ord` is **component-wise**, so `dir/sub` sorts immediately after `dir` and *before* `dir-x`, even though byte-wise `/` (0x2F) > `-` (0x2D). Descendants are therefore contiguous and `range + take_while(starts_with)` is sound.
- Adversarial set — siblings `dir!`, `dir-`, `dir.`, `dir0`, `dirA`, `dir~`, `dir\xff`, plus `dir/\x01x` and a non-UTF-8 `dir/\xff\xfe`, and probes `dir`, `dir/a`, `d`, `""` — **0 mismatches** against the full-scan result.

Root is handled correctly: `resolved` is empty, `Included("")` is the first key and `starts_with("")` is universally true, so the range degenerates to a full scan. The new test pins exactly this (`dir-x`/`dir2` siblings, `dir/sub/deeper` depth, `dir/*` direct-child depth, a non-directory, and root counting every entry). `is_directory()` is now hoisted out of the closure as `directory`, fixing the loop-invariant lookup too. **performance-005 and performance-006 → fixed.**

Scope noted: `review-followup-checks.json` records 4 checks at exit 0; hosted run 39964805125 passed **both** jobs at **148ef66**, which is *not* this head — I'm not treating it as covering eb767f3. No new VM result claimed; the finalization change rides on v261, which predates this commit.

## tests-001 — your pushback is correct, and it exposes a real gap

I was wrong to demand `ENOSYS`. `src/process/syscalls.rs:79` sets the allowed filter's default action to `Errno(PERM)`; `ENOSYS` appears only in the separate `clone3` fallback filter at `:90`. Asking for `ENOSYS` would have required changing production behavior to suit a test — exactly backwards.

You're also right that an ordinary `clone` success is not a positive control for CAP_SYS_ADMIN: a plain thread clone passes no namespace flag, so it exercises neither the mask nor the capability check.

**But inspecting the fixture turned up something more important than the test design.** The current assertion at `main.rs:2172-2180` shells out to `/usr/bin/unshare --user`. `unshare` is **not in `ordinary()` at all** — nor are `setns`, `mount` or `pivot_root`. So that call is rejected by the default-deny EPERM and **never reaches the `__NR_clone` mask rule**. The test named `NATIVE_SYSCALL_NAMESPACE_DENIAL_PASS` passes identically whether `MaskedEq(0x7e020080)` is correct, wrong, or absent. The mask is genuinely unexercised — and since EPERM is also what capability denial returns, the current assertion cannot distinguish the two mechanisms either.

I decoded the mask: `0x7e020080` is exactly `CLONE_NEWTIME|NEWNS|NEWCGROUP|NEWUTS|NEWIPC|NEWUSER|NEWPID|NEWNET`, with no stray bits (`mask & ~covered == 0`). Correct as written — just untested.

**Concrete proposal, benign, in-VM, no production change:**

The fixture is `#![forbid(unsafe_code)]`, so raw `clone` is unavailable. `rustix::thread::unshare` exists but is `#[deprecated]` in the pinned rev in favour of `unshare_unsafe`, and `thread` isn't in the fixture's feature list — so I'm *not* proposing it. Instead use `std::thread::Builder::spawn`, which reaches `clone` through libc with no namespace flags:

1. **Positive control (proves the filter permits ordinary clone):** `std::thread::spawn` must succeed — already present at `:2168`, just relabel it as the control.
2. **Mask discriminator:** spawn a child process running the fixture binary in a mode that calls `unshare` via the *already-allowlisted* path if one exists; if none does, the honest conclusion is that **the mask cannot be exercised from safe Rust in this fixture**, and the right fix is to record that limitation explicitly rather than fake coverage.
3. **Mechanism separation:** compare a namespace attempt made *with* CAP_SYS_ADMIN present (pre-`privileges.apply()`) against the same attempt after `syscalls.apply()`. If the first succeeds and the second returns EPERM, the denial is attributable to seccomp rather than capabilities. That ordering is available in the existing fixture flow and needs no new privilege.

My honest assessment: step 3 is the only genuinely discriminating test reachable today. If you'd rather not restructure the fixture ordering, then **recording "the clone argument-mask rule is unexercised" as an explicit M7 coverage gap is a legitimate alternative** — and better than the current test, which reads as coverage it does not provide.

## Changed dispositions

| ID | Was | Now | Basis |
|---|---|---|---|
| simplification-001 | fixed (w/ flaw) | **fixed, flaw resolved** | `R::LABEL` restores typed diagnostic |
| simplification-006 | accept | **fixed** | wrapper removed in eb767f3 |
| performance-005 | accept | **fixed** | `range` + hoisted `is_directory()` |
| performance-006 | accept | **fixed** | `range` + `take_while` |
| tests-001 | accept (my ENOSYS ask) | **accept, respecified** | EPERM is correct; see above |
| dependencies-013 | fixed | **fixed (strengthened)** | both hosted jobs passed at 148ef66 |

Counts move to **fixed 24 · accept 34 · reject 40 · open 9 · duplicate 5 = 111**.

## New grounded issue

**`NATIVE_SYSCALL_NAMESPACE_DENIAL_PASS` is misleading coverage** (`validation/rewrite-command-integration/src/main.rs:2172-2180`). It asserts a denial produced by the default-deny allowlist, not by the clone mask rule it appears to test, and EPERM cannot separate seccomp from capability denial. Low severity — the production filter is correct — but the test's name overstates what it proves. This is a *reviewer-proposed* action, not parent-accepted.

## Reviewer-accepted vs parent-accepted

- **Parent-implemented and I verified:** the LABEL restoration, wrapper removal, COPY ranges, matcher scratch buffers (af2b90f).
- **Reviewer-proposed, awaiting your decision:** the tests-001 respecification above, and the new misleading-coverage finding. Neither is accepted by you yet.

`policy-alternative.md` read as discussion only — I treat Aya/bindeps/precompiled BPF and the labeling exception as unimplemented, and infer no waiver of existing safety or qualification requirements from it. M7 remains at zero; the hosted job is rootless and qualifies neither distro, architecture, nor confidential hardware. No repo edits, no privileged operations.
