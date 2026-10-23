All 9 reconciliation batches are complete. Here is the full reconciliation.

## Independent verification I performed

Bash worked this time, so I ran the mechanical gate myself rather than citing your logs: `cargo fmt --all --check` exit 0, `cargo clippy --locked --offline --all-targets -- -D warnings` exit 0, `cargo test --locked --offline --all-targets` exit 0 (198 tests across binaries). **I withdraw my earlier "Mechanical: not run" framing as a project-level statement** — it was true of my session, not of the project, and I reported it in a way that implied the latter.

## Where I concede to your evidence

- **api-surface-003/004/005** — my agent excluded `validation/` as non-current. That was wrong and it invalidated the headline counts. My own sample: 5/12 alleged orphans live; critic R7's larger sample: **16 of 22 live (73% false-positive rate)**, mostly consumed by *inference* through public signatures, so they could not even be demoted without E0446.
- **api-surface-002** — `associated-visibility-rejected.rs` reproduces E0446; critic R3 independently reproduced it on a reduced model of `src/context/state.rs`. The proposed narrowing does not compile. Same disposition kills simplification-021.
- **errors-api-001 / unsafe-ffi-001** — critic R5 compiled an `execve(path, {NULL}, {NULL})` probe and ran it on this kernel: `child argc=1 argv0=`. Your kernel-source claim is empirically confirmed. No panic trigger exists.
- **unsafe-ffi-002 / security-008** — R5 read the pinned devtree checkout: `header.rs:26-28` rejects `< 40` bytes, `owned.rs:713-715` always emits the 40-byte header. Invariant holds.
- **tests-008** — serde-saphyr's `Options::default()` budget (depth 64, alias ratio, scalar caps) is preserved. "No limits configured" was false.
- **performance-003/008** — both rested on factual errors about buffering (devmap seeks before each tree write; `Stderr` is not line-buffered, and `LineWriter` batches to the last newline per chunk).
- **correctness-001 and documentation-001..005** — recorded as **fixed**, not as false positives. v259 reproduced the real failure in-VM; 27091e6 fixed it; 5ce229a fixed the docs.
- **"no proc-macro dependency"** — false; serde/thiserror/clap derives are in use. That premise is struck from the safety summary.

## Where I maintain disagreement

1. **performance-001 is still real, with your constraint intact.** Critic R3 verified `CheckedBlob::finish` recomputes *the same SHA-256 against the same descriptor digest* that `verify_references` already computed — that specific identity check is duplicated. Your point about preparation is separate and correct: the third read authenticates the *exact backing inode* and must stay. Those are different reads; only the first two overlap.

2. **tests-001 — I accept your rejection of a golden hash, and name the actual uncovered requirement.** The VM fixture checks *user*-namespace denial. The mask `0x7e020080` covers seven namespace flags plus `CLONE_NEWTIME`; the other six are never exercised. The meaningful check is behavioral — a command attempting each namespace flag is denied — not a filter digest or a mirrored allowlist.

3. **dependencies-012 is now concrete, not a vague tidy-up.** You were right to demand a named feature with an effect. R8 found it: `zstd` defaults enable `legacy` (992K of pre-1.0 frame decoders) and `zdict_builder` (184K dictionary API); the only use is `stream::read::Decoder` on cache-written blobs. `clap`/`regex`/`libbpf-rs` defaults survive inspection — I drop those.

4. **simplification-002 — I accept your framing correction.** A `/proc/self/task` count cannot prove lasting exclusivity. The win is a shared *diagnostic* helper across 10 sites, not an owning proof type. Sub-finding worth keeping: `src/network/namespace.rs:96` uses `.count()`, silently counting `Err` entries, unlike the other nine.

## Complete ledger — 111 findings

**FIXED (16)**: correctness-001 · documentation-001, -002, -003, -004, -005 · documentation-006 (wording already accurate; clarified) · tests-002 (independent SYS_ADMIN test) · dependencies-005, -006 (deny.toml, 4c1546b) · dependencies-003 (Cargo.toml rationale pointer) · dependencies-013 (workflow exists; *never executed* — that residue is M7, not this finding) · tests-011 (README:39 documents the tmpfs requirement) · simplification-010-adjacent inventory simplification (stable-checks) · api-surface-006 superseded · security-002 merged into security-001.

**REJECT (34)** — claim wrong, or barred by a preference/compiler:
performance-003, -004, -007, -008, -009 · security-001, -002 (dup), -003 is *open* not reject, -006, -007, -008 · unsafe-ffi-002 · concurrency-001, -002 · tests-005, -007, -008, -009, -010 · ownership-003, -004, -005, -006, -009 · api-surface-001, -002, -003, -004, -006 · simplification-007, -015, -019, -021, -026 · dependencies-001, -007, -009, -010 · documentation-007 · errors-api-004 (dup of simplification-020).

**ACCEPT (37)** — concrete change warranted:
- *High*: simplification-001 (generic `Finalization<R>`, ~130 LOC)
- *Medium*: simplification-004, -005, -006, -009 · performance-001 (dedupe identity hash only), -005, -006 · errors-api-005
- *Low*: simplification-002 (diagnostic helper + the `.count()` bug), -003 (path() only), -008 (annotation literal only), -011, -012, -013, -014 (two aliases only — `ExecutedBootstrap` has a live doctest), -016, -017, -018, -020, -022, -023, -024, -025, -027, -028, -029 · ownership-001, -007, -008, -010 · performance-010 (canonical; ownership-002 is the duplicate) · errors-api-001 (defensive only), -006, -007, -008, -009, -010 · unsafe-ffi-003 (comment) · concurrency-003 · tests-001, -004, -006 · dependencies-008, -011, -012.

**OPEN (8)** — specific missing evidence named:
- security-003 — needs VM-measured disclosure test against the qualified kernel
- security-004 — needs distro-workload compatibility evidence before narrowing splice/tee/vmsplice
- security-005 — needs a separate formatter capability/syscall profile + qualification
- performance-002 — needs proof a single pass preserves exact length, canonical padding and late decoder errors; `data_blocks` gives rounded geometry only
- errors-api-002, -003 — exit taxonomy deferred in DESIGN.md; needs an owned diagnostic representation preserving failure ownership
- api-surface-005 — needs a correctly-scoped recount including `validation/*/src` and inference positions
- tests-003 — CI exists but has never run on GitHub
- dependencies-002, -004 — RustSec git-source matching limitation; document rather than claim coverage

## Implementation shortlist

1. **simplification-001** — one generic `Finalization<R: PreparationStorage>`, using existing `unmount_until`/`retry_unmount` and `DirectiveStorage::validate`; `FinalizedVolume`/`FinalizedBootstrap` become aliases. Preserve ownership, failure diagnostics, exact original deadlines, concrete layouts.
2. **simplification-006** — delete the unused `Step::send_bootstrap_until`.
3. **performance-005/-006** — `BTreeMap::range` + hoist the loop-invariant `is_directory()`; preserve the direct-children depth filter.
4. **errors-api-005, -006, -007, -010** — `TryFrom<u64>` for `FilesystemSize`; manual `Display` using `cleanup_error()` with ownership untouched; drop the clock sample (no new duration cap); `.expect("Vec writer")`.
5. **dependencies-012** — `zstd = { default-features = false }`.
6. Dead-accessor removals: simplification-016, -017, -018, -024, -025, -027, -028.

Remaining disagreement after implementation: only performance-001's scope (identity hash dedupe, preparation read untouched) and tests-001's form (behavioral per-flag namespace denial, not a golden hash). No source, docs or Git changes made by me.
