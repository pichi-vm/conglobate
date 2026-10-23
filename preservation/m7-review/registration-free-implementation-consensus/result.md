The compatibility fix has both arms covered: character and block `mknod` denied with EPERM, `mkfifo` succeeding, and no sleep (FIFO `read` blocks for readiness; nonblocking `dd` checks reader absence). All points verified. Here is the reconciliation.

## Retractions — three of my claims were wrong

**B — "`MountPoint::open` dropped the tmpfs-magic check." Retracted; factually wrong.** `src/filesystem/attachment.rs:49-57` loops over **both** parent and target, rejecting unless `uid() == 0`, `mode() & 0o022 == 0`, and `fstatfs().f_type == 0x0102_1994`. Both opens use `NOFOLLOW`; the target also uses `O_PATH|DIRECTORY`. That is *stronger* than the old single-parent check, not weaker. I inferred the loss from the diff hunk header without reading the resulting function.

**A — "deleting `AssignedRoots::Bootstrap` would fail `label-operations`' `/mnt` loop." Retracted.** You are right and my *initial* distinction was correct: command-side labeling works under the global exception regardless of whether the trusted admission probe ran, so `label-operations` cannot detect probe omission. I collapsed two different properties. `label-operations-v1` proves command-level labeling on both roots; it does not close the differential gap.

**C — "README count drift." Retracted as an invalid comparison.** I compared `policy-removal-static` (195/51, uncommitted pre-migration state) against README's 193/50. Counts legitimately fell as retired policy/pin tests were deleted. I confirmed `review-boundaries-static` totals 243 across all binaries at this HEAD, and `NamespaceViewFailure`, `UserNamespace::idmap` and both stale policy comments (`src/storage/volume.rs`, `src/filesystem/attachment.rs`) are gone — all fixed in `a3c528a`.

## Differential coverage — agreed closed

I inspected both arms rather than accepting the summary. `fault.bpf.c` hooks `inode_setxattr`, matches the attribute name at exact length 17, requires `size == sizeof(expected)` **and** a byte-exact match of the probe's own label value, then CAS-records `first_device` and discriminates on `s_dev`. Counters: control `allowed=1, denied=0, passed=true`; negative `allowed=1, denied=1`, with `second-root-negative-v1/serial.txt:44` showing `restricted labeling validation: environment labeling support: Operation not permitted` and `:55 DISTRO_BUILD_EXIT 1` — no recipe marker.

The counterfactual is what makes this differential: omitting the `Bootstrap` arm means no `inode_setxattr` ever reaches the second device, so `denied` stays 0, the build proceeds past admission, and `build_rejected`/`command_not_admitted`/`output_absent` all fail. **This is a VM-only fault environment, not a production registration scheme** — no production hook, no map conglobate writes, no application Rust changed. Finding closed.

## mknod/FIFO compatibility correction — sound

`fifo_rules` builds `MaskedEq(S_IFMT)` against `S_IFIFO` on argument 2 (`mknodat`) and argument 1 (legacy amd64 `mknod`), which are the correct `mode_t` positions. I checked the discrimination arithmetically: only `S_IFIFO` matches; `S_IFCHR`, `S_IFBLK`, `S_IFDIR`, `S_IFREG`, `S_IFSOCK` and a bare `mode=0` all fail. `SeccompCmpArgLen::Dword` is right for `mode_t`. No capability added; default-deny preserved for everything else.

Evidence confirms both directions in one run (`descendants-v5`): `mknod ... c 1 3` and `b 1 0` both `Operation not permitted` with `test ! -e` confirming no node was created, then `mkfifo` succeeding. Synchronization is FIFO-based (`read -r token` blocks; nonblocking `dd` checks reader absence) — no sleep. `descendants-v2/v3` are correctly retained as the failed attempts that exposed the EPERM. This was a genuine source defect in the allowlist, not a fixture defect.

## Points D–G

**D — agreed, both.** Probe diagnostic context (root + operation for get/remove/open) is the right fix and does not weaken behavioral admission. On the `gate.start_until` race: I verified it independently — `await_admission_until(self, ...)` at `src/process/supervision/admission.rs:46` is by-value, so `Supervision<Admitted>` binds to the closure-local at `receiver.rs:155` and drops on `start_until` failure at `:156`, running `ControlSocket::drop`'s `shutdown(Both)` (`src/process/supervision.rs:40-45`) before the recovery block at `:166`. Your scoping is right: post-`09d698c` it can no longer orphan a shared mountpoint, so the residue is diagnostic loss only. Open, not yet fixed.

**E — accepted; I reclassify as optional defense-in-depth.** I could not identify a reachable path where `attach_output` failure is swallowed, both output owners are trusted, and the fault test now demonstrates distinct-device probing empirically. Requiring a second identity layer without a concrete reachable defect would be speculative. **Not an unresolved source defect.**

**F — accepted, with a correction to my wording.** "Self-attested" was the wrong word and I withdraw it; you explicitly disclaim production attestation. The accurate statement is narrower: independent rerun currently requires identified external artifacts, which `external-artifacts.json` discloses as `"durably_archived": false`. That is an open M8 obligation, not a claimed achievement.

**G — agreed.** Keep distinct diagnostics; no retries, no deadline renewal.

## On your preliminary responses to the evidence reviewer

I agree with your rejection of the "mirrors implementation" charge against the 64-GiB geometry and 64-MiB/4096-block formatter assertions — DESIGN 6.1 makes both normative, and inventory must record effective formatting. A test asserting a spec-required constant is a requirement test. Equally, recipe markers for a fixed committed fixture are legitimate test assertions; `status.raw` provides the independent supervisor-controlled channel. Moving the negative-assessment logic into `verify_missing.py` was the right reproducibility improvement regardless.

## Ledger

**Agreed / closed (7):** differential second-root probe (`92f528f`); command-level xattr regression (`label-operations-v1`); `MountPoint` cancellation-safety (`09d698c`) — *supersedes* the earlier `cli-timeout-v1` defect rather than duplicating it; mknod/FIFO compatibility (`346d793`); CBOR indefinite/NUL invariants and invalid-layout tag test (`a3c528a`); dead `idmap`/`NamespaceViewFailure` and stale comments removed; E reclassified as optional.

**Disputed:** none remaining. All three of my disputed claims are retracted above.

**Open (3):** probe diagnostic context (D, accepted, unfixed); `gate.start_until` channel-lifetime race (D, accepted, unfixed); durable artifact archiving (F, M8).

**Scope limitations:** native amd64 ordinary VM only — no arm64, TDX/SEV-SNP/CCA. Fault environment is VM-only and deliberately incompatible, proving probe reach, not production policy behavior. Broad legacy fault-injection harness migration still required. Neither full confinement nor platform qualification inferred from any run here. M2/M4–M6 gates remain 0/3; no adherence pass claimed, and milestone acceptance is yours.

No edits, no privileged operations, no additional agents, and no proposal to restore policy registration.