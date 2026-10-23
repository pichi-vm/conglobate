**Substantive confirmation: helper/cleanup success cannot substitute for command success.** The separation is enforced structurally at three levels:

1. **`ExecutionOutcome::success()`** (`src/step/execution.rs:24-26`) is `matches!(self, Self::Command(outcome) if outcome.success())` — a `SetupFailed` variant can **never** return true, and the inner `CommandOutcome::success()` must independently hold.
2. **`accept()`** (`:134-144`) takes `volume: R::Finalized` — i.e. finalization already succeeded — and *still* gates on `outcome.success()`. A successfully finalized volume with a failed command lands in the `Err` branch carrying `Cleanup::Complete(volume)`. Clean storage finalization is therefore necessary but **not sufficient**.
3. **`ExecutedVolume` cannot be forged** — the `compile_fail` doctest at `:34-37` proves no public constructor accepts an unrelated outcome, and its doc explicitly disclaims being an image seal.

The two values are also structurally unmixable: `Finishing`'s `(ExitStatus, ReapedChildren)` is the *helper's* status and never flows into `ExecutionOutcome`, which is built from `CommandResult`'s separately-preserved `CommandOutcome`. The corrected ARCHITECTURE text now states this exact chain.

## Ledger

**Agreed / closed (3)**

- **F1 — fixed.** DESIGN.md §1.1 now separates code independence from mandatory disposable-VM deployment, and rootless vs. privileged testing is scoped. This was the load-bearing one.
- **F2 — I accept your downgrade.** The top-level status is explicitly 0/3 and the work log is chronological, so a historical entry was not a current acceptance claim; "contradiction" overstated it. The added superseded marker removes residual ambiguity while preserving the original evidence — a better outcome than editing history.
- **F3 — retracted.** Two errors on my side: I cited `src/filesystem/attachment.rs` when `descriptor()` is at `src/storage/volume.rs:101`, and I treated an obligation as a defective claim. I verified there is no reachable violation: `VolumeView.root` is private, `descriptor()` is `pub(crate)`, the only `pub` method `in_namespace` consumes the view, and no `AsFd`/`AsRawFd`/`Deref`/`Into<File>` impl exists on `VolumeView` or `Mount`. Private fields and `pub(crate)` **are** Rust API boundaries; demanding an extra abstraction or test purely to satisfy wording would be exactly the invention you warned against.

**Parent-found finding — confirmed and correctly fixed (1)**

The outcome-ownership error was real and I missed it. Source disagreed with the old text: `Quiescent`'s payload is `DrainedHelper<'a>` and `Finishing`'s is `(ExitStatus, ReapedChildren)` (`src/step.rs:105-110`) — neither is the recipe command outcome. The corrected passage now names the actual chain (`CommandResult`→`CommandOutcome`→`ExecutionOutcome`), and I verified the non-substitution property holds structurally, not just by convention. This is a genuine catch that strengthens the architecture's accuracy.

**Disputed:** none remaining.

**Open (unchanged, correctly required by the architecture — not defects):** receiver start-send failure coverage; legacy fault-harness migration; distro/platform qualification (arm64, TDX/SEV-SNP/CCA); M8 durable artifact archiving.

## Corrected M2 architecture coverage

With F1 fixed, F3 withdrawn, F2 clarified and the outcome separation corrected, **all thirteen M2 deliverable categories now read as adequate and source-grounded.** The two categories I previously marked contradicted — dedicated-VM lifecycle and milestone-status consistency — are resolved, and "authority handles" returns to adequate on the corrected reading. My earlier verification stands: 61/61 owner-table types exist, state flows and `finish`'s infallibility match source, the filesystem schema matches `src/value.rs` exactly, no policy activation/disable states remain, all 22 traceability anchors exist with current/historical separation, and both preserved files verify against their checksums.

**I have no remaining architecture findings at this revision.**

This is architecture adherence only — it establishes nothing about M4–M8, implies no automatic milestone acceptance, and no test run here or previously counts as an adherence pass. The 0/3 streak and acceptance remain yours. No edits, no privileged operations, no new agents, no restoration of registration.