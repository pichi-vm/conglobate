That resolves it. Both production sites mint a **fresh cleanup/reporting budget at a terminal failure boundary** — `RuntimeFailure` carrying a 30s budget for the retained `mounts`, and the final completion report after the command has already exited. Neither renews an inherited mid-lifecycle cutoff, which is the pattern the design forbids. The four test-region sites are irrelevant.

## M4 revalidation — outcome

**No candidate defects established.** The broad sweep across `src/step*`, `src/domain*`, `src/storage*`, `src/isolation*`, `src/process*`, `src/network*`, `src/anchor*` (~12.4k lines) was interrupted at 48 minutes with zero returned findings. I am not restarting it, and **I do not treat that as a clean pass** — the great majority of that surface went unreviewed in this session.

### What I did establish directly

**Invariants verified by my own source reads (clean):**

- **No sleep-based readiness synchronization anywhere in `src/`.** `rg` for `sleep`/`nanosleep` returns only `clock_nanosleep`/`__NR_` seccomp allowlist entries — no busy-wait or retry-with-backoff used as a synchronization primitive.
- **No mid-lifecycle deadline renewal in the M4 scope.** Of ten `Instant::now() +` sites, four are inside `#[cfg(test)]` regions (`joining.rs:366,639`, `admission.rs:120`, `supervision.rs:204`) and the rest are fixed budgets at terminal boundaries: `runtime.rs:133,227,249` (cleanup of retained mounts), `receiver.rs:213` (post-exit completion report), `step/directives.rs:249` (`TEARDOWN_TIMEOUT`). None renews an inherited cutoff.

**Evidence I validated (not executed — these are your runs):**

- **19/19 linear-lifecycle cases pass at exactly this source**, including several previously-open branches: `sandbox-supervisor-death`, `sandbox-blocked-io`, `sandbox-writeback`, `sandbox-cleanup-timeout`, `sandbox-missing-cleanup`, `sandbox-gate-failure`. All carry `full_lifecycle_qualified: false`.
- **Source identity confirmed independently.** I recomputed SHA-256 for all 124–126 files in each of the 21 per-case manifests against current production `src/`: **zero drift**. My earlier "two non-matching cases" reading was my own key-name error (`matches_current_production`, not `matches`); I corrected it rather than reporting a false finding.
- **Preserved mechanical logs:** 275 tests across 18 result lines, 0 failures, 0 panics/errors in `environment-id-maps` and `linear-input-adherence`; Clippy clean. Per your instruction I make no claim to have run these.
- **The 111-finding ledger is fully dispositioned** — no `None`/unset `current_status` remains; 27 revalidated rejections, 21 implemented post-baseline, 15 revalidated fixed, plus the rest.

### What remains incomplete — genuine M4 blockers vs. later qualification

**Incomplete review (blocks a gate pass on evidence grounds, not a known defect):**
- Teardown/quiescence ordering and typestate sealing in `src/step/` — unreviewed this session, though verified correct in prior sessions at earlier revisions.
- `src/storage/` finalization, sealing integrity, loop/dm lifecycle, snapshot accounting — unreviewed.
- `src/isolation/` + `src/process/` escape surface, credential/seccomp confinement, SCM_RIGHTS handling — unreviewed.
- `src/network/` boundary consistency against "environment supplies namespace and DNS; binary does not configure enabled NAT/networking" — **unreviewed and the highest-value open question**, since that decision supersedes an architecture the module was built for. Whether leftover veth/NAT/nftables code is dead, reachable, or contradicting is unresolved.

**Later qualification, correctly not M4 blockers:** arm64 and TDX/SEV-SNP/CCA; distro boot (Alpine/NixOS); legacy fault-harness migration; M8 durable archiving. These are required *by* the architecture and tracked; treating them as M4 source defects would be wrong.

**Ledger items with concrete remaining actions** are Low-severity cleanups (`simplification-002` `.count()` Err-swallowing, `errors-api-002/003` diagnostic representation, `ownership-007/008/010`, `performance-002` single-pass proof). None is an M4 blocker on current dispositions.

### Bottom line

I established **no new defects**, and I verified two cross-cutting invariants plus the integrity of the evidence set. That is materially less than the M4 revalidation you asked for: four of the seven module groups received no review. CI passing at `9a70e50` with production Rust unchanged from `416ca11` strengthens the mechanical picture but does not substitute for the unreviewed source reading.

**M4 remains 0/3. No adherence pass, no completion claim, no blanket approval.** No edits, no commits, no privileged operations, no cargo, no restored guardian/policy/network architecture.
