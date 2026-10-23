Ledger validated: exactly 111 unique IDs, each appearing once.

## simplification-001 — verdict: implemented correctly, one real regression

I read both the old and new code, not just the explanation. Every preservation requirement you listed holds:

- **Original deadlines.** `Finalization::run` still checks `Deadline::check(self.deadline)` on entry, and the recursion and `retry()` thread the same `deadline` through — no renewal. `TeardownCompletion::unmount` still takes `self.deadline`. The single-volume/bootstrap asymmetry is preserved honestly: `Volume::unmount_until` ignores the parameter with a comment explaining the outer machine checks it, while `BootstrapVolumes::unmount_until` forwards it so the deadline is checked *between* the two filesystem operations.
- **Complete-but-invalid retention.** `Pending::Complete(volume)` is reconstructed into the returned failure on validation error (finalize.rs:89-97), identical to both originals.
- **Concrete retry ownership.** `Pending::Retry(R::UnmountFailure)` — the associated type is `VolumeUnmountFailure` vs `Box<BootstrapUnmountFailure>`, so each keeps its own owner; no boxing-to-dyn, no erasure.
- **Diagnostics history.** `self.diagnostics.followed_by(diagnostics)` on retry, `latest_only()` on a fresh unmount failure. The old single-volume `retry` already destructured through `*failure`; the shared one matches it.
- **Both bootstrap validations.** `BootstrapVolumes::validate` → `validate_snapshots()` (plural); `Volume::validate` → `validate_snapshot()`. Reachable via `NetworkedStorage::validate` forwarding to `R::validate`.
- **CommandResult.** `map_cleanup` copies `outcome` unchanged and `transpose` preserves it on both branches; the generic versions change only the type parameter.
- **Sealed/typed distinctions.** `StorageLayout` gained `validate` as `#[doc(hidden)]`; `DirectiveStorage` lost its duplicate and now calls `R::validate` at directives.rs:314. No cycle introduced. The four public names are aliases retaining concrete lifetimes, and `ExecutionStorage::Finalized`/`FinalizationFailure` still resolve to the distinct per-layout types.

**The regression:** `BootstrapFinalizationFailure` previously printed `"bootstrap storage finalization: {diagnostics}"`; it now prints `"step storage finalization: {diagnostics}"` because both aliases share one `#[error]`. An operator can no longer tell from the message which layout failed. No consumer greps the old string, so nothing breaks — but it is a real loss of diagnostic specificity, not a cosmetic rename. Fixing it needs a layout-supplied label (e.g. a `StorageLayout` associated const) rather than a manual `Display`.

Also: `src/step/bootstrap.rs` is now 41 lines, and the only thing left in it besides `with_bootstrap` is `send_bootstrap_until` — the dead method from simplification-006, still uncalled.

**Evidence I checked rather than assumed:** `finalization-checks.json` (4 checks exit 0, `vm_source_archive_matches: true`), v261 `verdict.json` (`passed: true`, `full_sandbox_qualified: false`), and the serial log showing `BOOTSTRAP_PAIR_BUSY_UNMOUNT_RETRY_PASS` across nonzero/timeout/nonzero-busy/success-busy/rollback variants — the retry path is genuinely exercised on both layouts. I independently re-ran rootless checks on the current tree: **198 tests, 54 doctests, strict Clippy, fmt — all exit 0.**

## API cleanup (a71c61c)

Correctly narrowed. You removed exactly the subset with grounded zero-caller evidence and kept what my critics found live: `ExecutedBootstrap` stayed (its `compile_fail` doctest is a consumer), `Supervision::wait_for_peer_exit_until` stayed (5 consumer callsites), `RestrictedCommand::execute` stayed, `CopyMode`/`ModeError` stayed public, and `receive_shapes` retained the socket-injection seam. I agree this implies nothing about the other orphan allegations — api-surface-004 remains rejected on its own evidence.

## Your six points

1. **performance-001** — agreed as you framed it. Marked accept, implementation pending; the exact-inode authentication at preparation is preserved, receipts for every non-scute component required, public verification ops stay independently usable.
2. **tests-001** — you are right and I was imprecise. Repeating `unshare` flags does not test the `clone` argument-mask rule, and a post-SYS_ADMIN-drop test can pass via `EPERM` even with a broken mask. Defined in the ledger: attempt `clone` with each namespace flag and require a seccomp errno (`ENOSYS`), not `EPERM`, plus a positive control — a `clone` with no namespace flag that must **succeed**. That separates the two mechanisms. In-VM only.
3. **errors-api-001** — **I withdraw my "accept as defensive."** Marked reject. I found no invariant gap; one-line cost is not a justification.
4. **performance-010** — marked fixed, verified in my own run.
5. **CI** — all "never run" claims withdrawn; dependencies-013 is fixed, hosted run 39964805125 passed its Rust job. I infer no qualification from a rootless job, and I note the dependency job result is still pending.
6. **Aya/bindeps/precompiled BPF** — noted as discussion only; no policy migration or waiver assumed.

## Counts (computed from the array)

**fixed 20 · accept 37 · reject 40 · open 9 · duplicate 5 = 111**

Duplicates: `errors-api-004`→`simplification-020`, `ownership-002`→`performance-010`, `performance-007`→`security-001`, `security-002`→`security-001`, `security-008`→`unsafe-ffi-002`.

The full JSON array is at `/var/tmp/conglobate-rewrite-tmp/ledger.json` (outside the repo — I made no repo edits). Each row carries id, status, canonical_id, evidence and remaining_action.

## Remaining disagreement

1. **simplification-001 Display regression** — the only new concrete item from this review. The bootstrap-specific failure text was lost in the merge; I'd restore layout-specific labelling.
2. **simplification-006** — `send_bootstrap_until` survives at `src/step/bootstrap.rs:24` with zero callers and duplicates `ExecutionStorage::send`'s bootstrap branch. Still accept.
3. **Nothing else.** Every other reconciliation-1 dispute is resolved: I conceded api-surface-002/003/004/006, errors-api-001, unsafe-ffi-002, tests-008, performance-003/008, and the dependency items; you conceded performance-001's initial scan and tests-001's behavioural coverage.

M7 remains at zero; no adherence pass is claimed. Unresolved action items are the 37 accepts and 9 opens, kept separate from the 20 fixed.
