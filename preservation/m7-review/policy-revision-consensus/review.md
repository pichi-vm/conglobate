All source claims verified. Per-item dispositions follow.

## D1 — Agreed: justified deletion, no re-homing

**I accept your correction and withdraw the "missing independent authorization boundary" characterization.**

I traced the provenance and your argument holds. `Volume::view` (`src/storage/volume.rs:322-336`) calls `self.scope.verify_current()?` and then `open_tree` on `&self.resources.root` — a **private** field of the volume the `Step` already owns. `VolumeView.root` is private too (`:95-99`), reachable only through `pub(crate) descriptor()`. So the view is derived from the owned volume's own FD, not from a caller-supplied path or an arbitrary root FD.

That makes `namespace_for_root` structurally incapable of being the authorization boundary I described. Both the pin (`:339-341`) and the view derive from the *same* `self.resources.root`, so the dev/ino comparison asks "is this volume's root the one I pinned from this volume's root?" — it reconciles the policy-side target list against the storage owner. It is consistency-checking within the policy registry, not independent validation of recipe-assigned selection.

The decisive point is yours: once the policy owner is gone there is **no second target list to reconcile against**. A re-homed registry would have to be invented for the sole purpose of checking the owned volume against itself, which is circular. The sealed `ExecutionStorage` impls sending views of `Step`-owned volumes already carry the property by construction, and typestate enforces it at compile time rather than at runtime.

My bootstrap observation — that `storage.rs:95` discards its result — is explained by the same reasoning: it was reconciling the *second* pin, not asserting a second independent fact. With no pins, nothing is left to reconcile.

**Requirement satisfied:** ARCHITECTURE explicitly documenting the removal as redundant plus requiring sealed owned-volume handoff with no arbitrary caller root FD is exactly the right disposition. **D1 closed — not a blocker.**

## D2 — Agreed: enforcement clause is adequate

The revised 4.5 now names the mechanisms rather than stating a prohibition: private mount construction, pivot_root and old-root detachment, confined runtime mount targets, private proc, non-dumpable trusted helpers, descriptor closure/audit before image code, pipe-relayed stdout/stderr, assigned null stdin, read-only immutable views, and seccomp/capability restrictions against new mounts or namespace joins.

I verified the two source claims: `src/isolation/receiver.rs:106-107` sets `Stdio::piped()` for both streams, so the image never inherits a supervisor log file handle; `src/isolation/root.rs:77` opens `/dev/null`, `:146` dup2s it onto stdin, and `:186,190` runs `check_descriptors()` before exec with the comment confirming the null input was installed and closed by `restrict`. The clause matches implemented behavior.

The closing requirement — "Validate this combined boundary again with the global exception active" — is the right obligation, since the old evidence was gathered with target guards narrowing the blast radius. **D2 closed as an architecture concern.**

## 4.6 ordering gap — closed

My gap was that nothing ordered the probe against the cgroup handshake. 4.6 now specifies: complete cgroup-join acknowledgment → launch restricted leaf → apply capabilities and seccomp → probe in the contained leaf → close all probe handles → readiness/admission, with both bootstrap filesystems probed via anonymous inodes and handle closure preceding the final descriptor audit.

That is the strictest sensible order: the probe runs under exactly the credentials the command will have, inside the containment, and leaves nothing behind for the descriptor audit to find. ARCHITECTURE carrying the same placement means the two documents cannot drift. **Closed.**

## E1 — Superseded; I accept v3 and will not reuse v2

`c49a11a` preserves `global-label-v3`. I verified: its `serial.txt` contains `ANONYMOUS_LABEL_PROBE_PASS` followed by `GLOBAL_EXACT_LABEL_BOUNDARY_PASS`, both `NO_POLICY_DENIED` controls (before load and after pin removal) are present, `FIXTURE_EXIT 0`, and `verdict.json` is `{mechanism_passed: true, production_lifecycle_validated: false}`. The v3 `probe.c` is byte-identical to the working tree, so the preserved artifact matches what ran.

v2 is intentionally unchanged — its `serial.txt` has zero `ANONYMOUS` occurrences and its last commit is `ef4ee59`. **I will cite v3 for anonymous-probe claims and v2 only for the original boundary run; no cross-use.** E1 closed.

## Two corrections I accept

**Capability-gate scope — my "implies" was too strong.** I said zero-effective-cap success implies success under production's retained capabilities. That is only valid holding mount, seccomp and LSM configuration fixed, and the probe differs on all three (no idmapped mount, no seccomp, `lsm=capability,bpf` only). The correct narrower claim: **v3 evidences the capability gate specifically — that the exception delegates the generic capability check such that a zero-effective-capability subject can set `security.selinux` and cannot set other `security.*` names.** It evidences nothing about the composed production environment. Production retest remains required.

**`ret` semantics — my "preserve earlier denials" phrasing was loose.** In `inode_xattr_skipcap`, `ret` is a prior **skipcap claim/result**, not an ordinary LSM deny-convention return. `if (ret) return ret;` propagates whatever an earlier hook concluded about skipping the capability check; nonzero there is not universally a denial. The accurate statement: **the hook preserves the prior skipcap result rather than overriding it, and only returns its own claim when no earlier hook made one.** 4.4's revised wording ("preserve earlier LSM results" rather than "denials") already reflects this — I should have read the change as deliberate.

## Consensus and blockers

**No remaining architecture blockers.** D1, D2, the 4.6 ordering gap and E1 are all closed. The contract is internally consistent, net-subtractive, and its evidence claims are correctly scoped.

I explicitly distinguish, and do **not** count as architecture failures, the unimplemented obligations that properly follow design acceptance: the production probe and its failure tests, policy-module and BPF-build-dependency removal, the `Registered` rename, and revalidation of the composed boundary with the global exception active. Production still threading `LabelPolicy<BoundDisabled>` through `ExecutionStorage::send` is expected at this stage, not a defect.

Noted and agreed: no new probe typestate or protocol is planned — `OwnedFd` plus the existing readiness sequencing suffices, with RAII giving handle closure for free. Production implementation follows three clean design passes; `production_lifecycle_validated: false` remains the correct status, and no Arm or confidential-platform qualification is implied.
