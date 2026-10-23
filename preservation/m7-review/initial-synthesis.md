All 14 critic batches returned. Synthesis follows.

```
## /review — src/ (137 files, 28,594 LOC, crate `conglobate`, bin+lib, publish=false)
```

**Primary operations**
- `Pipeline::run` — src/pipeline.rs:53 — root-only build driver: capture → resolve → workspace → bootstrap/firmware/carapace/inherited → cleanup → publish
- `HelperInvocation::parse`/`execute` — src/execution/helper.rs:148,:282 — argv-dispatched trusted re-exec entry points
- `CommandExecution::execute` — src/step/execution.rs:152 — one untrusted recipe command under namespaces + BPF-LSM admission + drain
- `Formatting::format` — src/formatter/execution.rs:117 — privileged trusted mkfs/ext4 launch
- `Inventory::decode`/`encode` — src/inventory.rs:25,:101 — deterministic CBOR v2 + SHA-256 binding
- `AssembledImage::publish` — src/output.rs:172 — atomic staged publication anchored to a parent fd

> ⚠️ **The tree changed mid-review.** HEAD advanced `b746f69` → `27091e6` and 17 files gained uncommitted doc edits. All seven documentation findings and the one logic finding were overtaken by that concurrent work — listed under *Already fixed* below, not as open issues.

### 🔴 Critical (0)
None. No `unsafe` anywhere (forbid verified three ways), BPF map ABI matches `labels.bpf.c` exactly, SCM_RIGHTS handling sound at all eight transports, no synchronization sleeps anywhere.

### 🟠 High (3)

1. **[Tests] src/process/syscalls.rs:21–129 — seccomp filter construction has zero host unit tests**
   • Problem: `compile()` builds the default-deny policy confining untrusted commands — four argument-rule allowlists plus the clone mask `0x7e020080` — in pure unprivileged code, yet nothing asserts it; the mask is explained only by a prose comment.
   • Fix: pin the compiled program (golden digest) and assert the mask equals the OR of the named `CLONE_NEW*` constants.

2. **[Tests] src/process/privileges.rs:22–114 — capability-drop set asserted only in the VM harness**
   • Problem: `filesystem()` is a pure constructor and `allowed()` exposes the set, so adding `SYS_ADMIN` would pass `cargo test` unnoticed.
   • Fix: pin `filesystem().allowed().bits()` and assert SYS_ADMIN/SETPCAP/SYS_MODULE/BPF absent. (The drop itself legitimately stays harness-only.)

3. **[Simplification] src/step/finalize.rs:10–153 — duplicate Pending/Finalization state machine**
   • Problem: `finalize.rs` and `step/bootstrap.rs` hold two copies of the same unmount/validate/retry machine; every difference is already covered by an existing trait hook (`unmount_until`, `retry_unmount`, `DirectiveStorage::validate`), and both `Complete` arms return `R::Unmounted`.
   • Fix: one generic `Finalization<R: PreparationStorage>`, mirroring the existing `ExecutedStorage<R>` precedent — erases no typestate. ~130 LOC.

### 🟡 Medium (10)

1. **[Simplification] src/step.rs:277–362** — three `cancel_until` impls byte-identical bar `retry_until`; factor the Step-rebuild into a private helper, keeping all three typestate entry points. ~50 LOC.
2. **[Simplification] src/pipeline/bootstrap.rs:301–343** — `BootstrapBuild::reject` / `CarapaceBuild::reject` are arm-for-arm identical except `discard` vs `discard_pair`. ~45 LOC.
3. **[Simplification] src/isolation.rs:84–92** — single-threaded-helper guard hand-rolled at 10 sites across 3 modules; one owning proof type collapses it. ~45 LOC. (`network/namespace.rs:96` uses `.count()`, silently counting `Err` entries.)
4. **[Simplification] src/step/bootstrap.rs:155–173** — `Step::send_bootstrap_until` has zero callers and is a verbatim subset of `ExecutionStorage::send`'s bootstrap branch: a live drift hazard. ~35 LOC.
5. **[Simplification] src/storage/loopback.rs:39–112** — `LoopAttachment` and `ReadOnlyLoop` duplicate the whole loop attach/detach sequence incl. the ENXIO confirmation, in code that is itself untested. ~40 LOC. (Keep the `sync_all` difference parameterised.)
6. **[Performance] src/input.rs:157** — every scute layer's SHA-256 identity check runs twice (`verify_references` then `CheckedBlob` in `verify_chain`), then a third read for the verity tree. The verity work is distinct; the identity hash is genuinely duplicated.
7. **[Performance] src/cache/chain.rs:126–144** — compressed scutes fully decompressed twice: once into `io::sink()` to measure, once for the root. Single-pass is feasible when `data_blocks` is present.
8. **[Performance] src/copy/source.rs:47–55** — `descendants()` scans the whole context `BTreeMap` instead of the range under `self.resolved`, and re-evaluates the loop-invariant `is_directory()` map lookup per entry.
9. **[Performance] src/copy/source.rs:112–122** — wildcard selection scans the full key set per candidate; O(candidates × entries) per component.
10. **[Deps] deny.toml:28–32 + deny.toml:9–15** — two self-contradictions that fail on the project's own tree: `unknown-git = "deny"` with only `pichi-vm` allowlisted while Cargo.toml pulls rustix from `bytecodealliance`; and the five-license allowlist omits `BSD-2-Clause`, which is libbpf-sys's own declared license.

### 🟢 Low (18, abbreviated)
`security-001` unbounded zstd measuring pass (self-inflicted, single-tenant VM) · `security-003` /proc masking omits kallsyms/sched_debug · `security-004` vmsplice/tee in the allowlist · `security-005` formatter runs as initial-ns root under chroot · `security-006` `StagedDirectory::publish` lacks a type-level verification gate · `security-008`/`unsafe-ffi-002` fixed-offset DTB slice (devtree *does* enforce the 40-byte header — defensive nit) · `unsafe-ffi-001` argv slice panics at argc==0 (unreachable from untrusted side) · `unsafe-ffi-003` BPF config map update non-atomic (no concurrent reader exists) · `concurrency-001` startup-failure path detaches the diagnostic relay · `concurrency-002` drain can report TimedOut at the exact boundary · `concurrency-003` three `enter()` wrappers mint a fresh 30s deadline (no production caller) · `errors-api-002/003` `BuildError::Execution(String)` and `InventoryError` flatten typed failures · `errors-api-004` COPY mode AST re-exported but unused · `errors-api-009` `unreachable!()` in network cleanup (genuinely unreachable) · `api-surface-001/002/003` visibility leaks; only `firmware` and `transform` of the nine pub modules have zero external consumers · `dependencies-003/004` rustix linked twice from two sources · `tests-004` 0xaa64 aarch64 arm never exercised · `tests-006` `InterfaceName` boundaries unpinned.

### Mechanical
- fmt: **not run** — `cargo` denied by permission mode
- clippy: **not run** — same
- check: **not run** — same
- tests build: **not run** — same

This is the biggest gap in the review: no compiler or lint signal was obtained. `.github/workflows/checks.yml` does run fmt, clippy `--all-targets` and `cargo test --locked --all-targets` on every push, so CI covers it — but this review did not.

### Top simplification wins
1. src/step/finalize.rs + src/step/bootstrap.rs — one generic `Finalization<R>` — ~130 LOC
2. src/step.rs:277–362 — factor the shared Step-rebuild out of three `cancel_until` impls — ~50 LOC
3. src/isolation.rs + 9 sites — one `SingleThreadedHelper` proof type — ~45 LOC
4. src/pipeline/{bootstrap,carapace}.rs — one generic `StageFailure` recovery walk — ~45 LOC
5. src/storage/{loopback,verity}.rs — one private loop-device owning type — ~40 LOC

### Already fixed (overtaken by concurrent work)
All seven documentation findings — stale "No CARAPACE support yet", crate-root "pipeline assembly remains under implementation", undocumented `Step` transition order (a diagram now exists; ARCHITECTURE.md state names corrected), `Volume` Drop lazy-detach, and the claim that no `# Errors` section existed anywhere (seven now do). Also `correctness-001`: commit `27091e6` sorts the formatter inventory by wire paths, which is exactly the String-vs-Path ordering mismatch reported.

### Not checked
- **fmt/clippy/check/test-build** — `cargo` denied; no compiler signal at all
- **Line/branch coverage** — cargo-llvm-cov and cargo-tarpaulin are installed but unrunnable; all test judgments are static reads
- **cargo audit / cargo deny** — denied; advisory status of the ~230-crate tree is *unknown, not clean*. The nine git deps are rev-pinned (good) but sit outside RustSec matching regardless
- **Confidential-computing (SEV/TDX/CCA) and Arm/aarch64 hardware** — unavailable; the firmware fixtures are honestly scoped as structural parser tests and do not substitute. The 0xaa64 match arm is unexercised
- **Privileged runtime behaviour** — BPF load/attach, dm-verity, loop devices, namespaces, netlink: no VM harness run
- **Dependency internals** — rustix, libbpf-rs, seccompiler, goblin, devtree, devmap, serde-saphyr reviewed only at the call site

### Filtered by critic
- Rejected: 17 — notably `performance-008` (wrong about `LineWriter`, which batches to the last newline per chunk), `tests-007` (the glob matcher is a forward NFA, so the claimed quadratic blowup is impossible), `tests-008` (serde-saphyr's defaults already carry depth/alias/size budgets), `dependencies-009` (`=` pins are defensible anti-drift for seccomp/syscall layers given a committed lock and `--locked`), `dependencies-001` (a review-coverage gap, not a code defect), `simplification-007` (the `compile_fail` doctest *is* a consumer encoding a typestate invariant).
- Downgraded: 24
- Merged as duplicates: 6

### Clean
Correctness/logic (one finding, since fixed — the integrity, path, teardown and process-lifecycle paths read clean), Unsafe/FFI (no application unsafe; BPF ABI exact), Concurrency (no synchronization sleeps; CLOEXEC discipline re-audited before exec).

### Verdict
**Needs attention** — no Critical, no High in Unsafe/Security/Correctness. The two security-invariant test gaps (seccomp allowlist, capability set) are both cheap unprivileged tests and are the highest-value next step; the `deny.toml` self-contradictions are a one-line fix each. Before accepting, re-run the mechanical gate locally, since this review obtained no compiler signal.
