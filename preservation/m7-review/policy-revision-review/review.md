I have what I need. Findings follow.

## Summary

The contract is coherent and the simplification is real. The mechanism evidence supports the *kernel behavior* claims it makes, and the README scopes itself honestly. My findings are mostly **unimplemented obligations** and **documentation gaps**, with two I'd call design bugs and one evidence-hygiene problem.

---

### Design bugs

**D1 — `namespace_for_root` is a hidden authorization check, not just a lookup; nothing in the revision replaces what it enforced.**
`src/step/execution/storage.rs:51,94-95` calls `policy.namespace_for_root(&self.root_metadata()?)` before every storage handoff. That is not a convenience accessor: `src/policy/bound.rs:175-188` iterates pinned filesystems, compares `(dev, ino)` against the volume root, and **errors with `"step volume root is not pinned by its policy"` if no pin matches**. It is a positive proof that the volume about to be sent is one the supervisor actually assigned.

ARCHITECTURE.md's revision says "Storage handoff takes that handle directly; the owned volume determines which mount is transferred. No policy scope/map lookup is required to prove that ownership." That is true of *namespace* ownership — the `UserNamespace<PinnedNamespace>` is needed only for `MountAttrParamFlags::IdMap` at `src/filesystem.rs:134`. But it silently drops the dev/ino cross-check. Bootstrap is the sharp case: `storage.rs:94-95` validates **both** tool and output roots, and the second call's result is deliberately discarded — it exists purely as a check. Removing the policy module deletes that check with no stated replacement.

Not fatal (the owning `Volume` is already typestate-proven), but the revision should say explicitly whether the dev/ino assertion is **intentionally dropped as redundant** or must be **re-homed onto the storage owner**. Right now it is removed by omission.

**D2 — 4.5 states the reachability requirement but no document assigns the obligation to a mechanism.**
SANDBOX-DESIGN 4.5 and DESIGN.md correctly identify the new integrity model: `xattr_permission` delegates `security.*` before ordinary `inode_permission`, so write-mode denial is not a boundary, and the probe demonstrates this (`setxattr` succeeds on a `0444` file, line 42). The replacement boundary is "inode/descriptor reachability and read-only views."

But under the old design, the target guards bounded damage to *registered superblocks*. Now **any** inode a command can reach is label-mutable. The documents say "must not reach mutable supervisor/cache/context/control inodes" without naming what enforces it — `pivot_root` confinement, mount flags, and descriptor closure are each covered elsewhere, but no section ties them together as *the* labeling integrity boundary. That is the single largest change in the threat model and deserves an explicit enforcement clause, not a prohibition.

---

### Admission-validation placement — appropriate, with one gap

Placement is right. SANDBOX-DESIGN 4.6 requires validation **under restricted command credentials**, before admitting recipe commands, with no capability fallback, and explicitly rejects inferring support from a pin pathname or a supervisor-root write. The probe evidence justifies all three exclusions: the loader pins at `/sys/fs/bpf/label-exception`, and `rm`-ing that pin restores denial (`serial.txt`), so pathname presence genuinely proves nothing.

**Gap:** 4.6 says validation happens before admission, and 4.7 says "cgroup joining and admission handshakes remain necessary," but neither orders the probe relative to the cgroup handshake. `Supervision<Registered>` (`src/process/supervision/registration.rs:26-29`) is an *acknowledgment* that the helper placed itself in its domain — it is not a labeling capability. If the probe runs in the trusted helper after `join_domain_until` but before release-to-execute, that should be stated; otherwise the implementation has latitude to probe at a point where credentials differ from the command's.

---

### Namespace vs policy ownership — correct

Sound and well-evidenced. The namespace handle's only non-policy consumer is the idmapped-mount `set_param_flag(IdMap(namespace.as_fd()))`. Moving it to the isolation module is right, and `UserNamespace<PinnedNamespace>` already lives there (`src/isolation/user.rs`).

Removing filesystem pins is also justified **as stated**: `src/storage/volume.rs:339-341` creates them and `src/policy/bound.rs` is their only consumer, so they are policy-lifetime artifacts guarding numeric target-identity reuse — a race that no longer exists without maps. Caveat: this is exactly the mechanism D1 depends on, so decide D1 first.

---

### Cgroup handshake preservation — correct and worth keeping explicit

ARCHITECTURE.md's "rename `Registered` to describe containment rather than deleting its acknowledgment or descriptor closure" is the right call. The handshake at `registration.rs:64-86` verifies the transferred FD is genuinely cgroup2 and closes it after self-placement; that is independent of labeling and must survive. Good that the revision calls out the rename rather than treating the state as policy residue.

---

### Limitations of the zero-effective-cap one-UID mechanism test

The README is honest, and I confirmed each limitation is real rather than boilerplate:

- **One UID, no idmapping.** `chown 65536:65536` plus `--map-root-user` gives a 1:1 single-ID map with the file already owned by the outer UID. Production uses a *range* through an idmapped mount (`mount_setattr` + `IdMap`). The probe asserts `inner==0 && outer==65536 && count==1` — it does **not** exercise the idmapped translation path at all, which is where label operations could behave differently.
- **Zero effective caps is stronger than production, in one direction only.** `caps == 0` with bounding/inherit/ambient cleared is a *more* restrictive subject than `CommandPrivileges::filesystem()`, so success here implies success there. But it proves nothing about interaction with the capabilities production *retains* (CHOWN, DAC_OVERRIDE, FOWNER…).
- **Not the command sandbox.** No seccomp (`CommandSyscalls` allowlists `setxattr`/`fsetxattr`/`removexattr` at `src/process/syscalls.rs:185-188`, untested here), no pivot_root, no network isolation, no PID namespace, no drain, no bootstrap dual roots.
- **`lsm=capability,bpf`.** A deliberately minimal stack; a qualified build kernel with more LSMs could order differently. 4.4's "preserve earlier LSM results" is untested.

The `ret` short-circuit in `labels.bpf.c` (`if (ret) return ret;`) correctly preserves prior denials, but with only `capability,bpf` loaded, nothing ever sets `ret` nonzero in this run.

---

### Disposable-file xattr probe — implementable simply, and the working tree now shows it

You asked whether this can be done without exposing descriptors or leaving artifacts. **Yes**, and `probe.c:31-40` (changed on disk since my first read) now demonstrates exactly the right construction: `open("/assigned", O_TMPFILE|O_RDWR|O_EXCL|O_CLOEXEC, 0600)`, then `fsetxattr`/`fgetxattr`/`fremovexattr`, then `close`. `O_TMPFILE` has no pathname, `O_EXCL` prevents linking it into the namespace, `O_CLOEXEC` stops inheritance across the command exec, and the inode is reclaimed on close — no output artifact, no reusable name, nothing for a command to race.

In production Rust this needs no new machinery and no unsafe: `rustix::fs::open` with `OFlags::TMPFILE | RDWR | EXCL | CLOEXEC` (`TMPFILE` is present in the pinned rev), then `rustix::fs::{fsetxattr, fgetxattr, fremovexattr}` — all safe, all already in the enabled `fs` feature. An `OwnedFd` gives RAII cleanup for free, satisfying the destructor preference. The positive/negative control pair is just `Ok` vs `Err(EPERM)`.

**Two caveats:** `O_TMPFILE` is filesystem-dependent (fine for ext4, must be re-checked when qualifying other filesystems per 4.1), and the probe must run on **each** assigned writable root — both bootstrap roots, as 4.5 requires — not once globally.

---

### Unnecessary new machinery

I found **none** in the contract. No new callback, state flag, forgeable receipt or registration protocol is introduced; the revision is net-subtractive. The one thing I'd flag as *potentially* unnecessary is adding a new typestate for probe-completed: the existing `SetupBudget`/release-to-execute sequencing can carry it as a precondition without a new sealed state.

---

### Evidence hygiene

**E1 — `probe.c` and `run.py` in the working tree are newer than the preserved evidence.** `preservation/m7-review/global-label-v2/serial.txt` contains `GLOBAL_EXACT_LABEL_BOUNDARY_PASS` but **not** `ANONYMOUS_LABEL_PROBE_PASS`, and the preserved `probe.c` differs from the working copy. `run.py:98` now *requires* that marker. So the O_TMPFILE probe is **unvalidated** — the preserved run predates it. Per "historical passes are reopened, not silently reused," this needs a fresh run before the anonymous-probe approach is treated as demonstrated. The design conclusion is unaffected; only the evidence claim is.

---

### Classification

| | |
|---|---|
| **Design bugs** | D1 (dropped dev/ino assertion), D2 (unassigned reachability enforcement) |
| **Unimplemented obligations** | 4.6 production probe + failure tests; probe/cgroup ordering; per-root probing incl. bootstrap pair; policy-module and BPF-build removal; `Registered` rename |
| **Evidence** | E1 (anonymous probe unvalidated); mechanism run does not cover idmapping, seccomp, retained caps, dual roots, richer LSM stacks |

Production still implements the old lifecycle — I verified `LabelPolicy<BoundDisabled>` is still threaded through `ExecutionStorage::send`. Nothing here is a claim about implemented behavior, and `global-label-v2`'s `production_lifecycle_validated: false` is the correct self-assessment. No edits, no privileged operations, no VM runs.
