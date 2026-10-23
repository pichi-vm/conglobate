The caller stringifies the error immediately before guaranteed process exit — so the new variants' retained owners are irrelevant to it, and the asymmetry between the two arms is intentional. I have enough to report.

## /review — registration-free implementation, 522011a..c1073b9

**Scope reviewed:** 46 files, ~1,900 net lines deleted. `src/isolation/labels.rs` (new), receiver admission lifetime, step preparation/teardown, namespace ownership removal, bpffs removal, build-dependency removal, test/evidence adequacy, ARCHITECTURE.md vs source. Four parallel reviewers plus my own verification. **No milestone gate is claimed passed.**

### Verdict on the user's two explicit questions

**1. Can storage finalization be admitted before descendants stop? No.** I verified the chain directly at `src/step/teardown.rs:154-176`: `TeardownCompletion` is constructed at exactly one site (`:175`, from `Phase::Complete`), `Complete` is produced only by the `Finishing` arm (`:166-171`), and `Finishing ← Quiescent ← Draining ← Reaping` with one producer each. `Finishing`'s resources are `(ExitStatus, ReapedChildren)` from `DrainedHelper::finish`, so the state is literally uninhabitable without a completed reaper drain. The deleted `Phase::Policy`/`PolicyRetry`/`Pins` sat *after* the drain — removing them shortened the tail, not the head. Failure recovery maps backward only. The preparation-rollback path also drains before `Phase::Unmount` (`src/step/preparation.rs:280-283`, `:321-332`).

**2. Is the assigned root trusted or inferred from image content? Trusted.** The tag originates at `src/process/supervision/volume.rs:140-146` from `self.output.is_some()`, which is set only by the SCM_RIGHTS packet variant from the supervisor channel, re-encoded into the child's argv by the trusted parent (`src/execution/helper.rs:379`), and decoded with canonical-reencode validation (`:282-287`). Untrusted command code never spawns the helper. For bootstrap, `/mnt` is supervisor-created (`src/storage/volume.rs:452-465`) and the output filesystem is `move_mount`ed over it (`src/filesystem/attachment.rs:188-205`) before any image code runs — both using `BENEATH|NO_SYMLINKS|NO_XDEV`.

### Findings

**High — no evidence that the bootstrap second-root probe actually ran.** `src/isolation/labels.rs:26-34` makes the `/mnt` probe conditional. Nothing asserts it executed: deleting the `if let Self::Bootstrap` arm entirely would leave `bootstrap-unpinned-v1` still emitting `BOOTSTRAP_TWO_ROOTS_*_PASS` and `{"build_passed": true}`. Both negative runs (`label-admission-off-v2`, `label-unpinned-off-v1`) use the single-root `tests/distro/labeling/recipe.yaml`, so the second root has **no negative counterpart**. This is SANDBOX-DESIGN 4.6(e), currently unfalsified.

**High — no test asserts labeling from the untrusted command's perspective under the new code.** `tests/distro/labeling/recipe.yaml` is a bare `printf`; `COMMAND_LABEL_ADMISSION_PASS` proves only that a command ran. The probe runs earlier, in trusted setup, on an anonymous inode the command never sees. `tests/distro/fedora/recipe.yaml:31` *does* exercise `setfiles`/`getfattr` command-side — but the only preserved run of it (`fedora-build-v3`) is from commit `0d29786`, which **predates** `522011a`, i.e. it validated the old scoped-policy code. Re-running that recipe post-migration would close both this and much of the previous finding.

**Medium — `src/isolation/labels.rs` cannot self-evidence "two distinct filesystems."** It is the only `"mnt"` resolution omitting `NO_XDEV` (cf. `filesystem.rs:192`, `attachment.rs:197`, `volume.rs:462`). The omission is *correct and required* — crossing into the output mount is the point. But consequently, if the output mount were ever absent, the probe would validate the tool filesystem twice and still report success. An `fstatfs`/`st_dev` inequality check between the two handles would make 4.6(e) self-evidencing. I found no path where `attach_output` failure is swallowed, so this is defense-in-depth, not a live bug.

**Medium — inconsistent error context in the probe.** `labels.rs:69-70` wraps only `fsetxattr` with `"environment labeling support: {e}"`. The two most diagnostically interesting failures — `fgetxattr` returning `ENODATA` (attribute silently dropped) or `ERANGE` (oversized stored value) at `:71`, and `fremovexattr` at `:75` — surface as bare errnos. Neither identifies *which* root failed; the message is identical for `/` and `/mnt`.

**Medium — orphaned CBOR invariant.** `tests/namespace_records.rs` (deleted) asserted indefinite-length array rejection for the 256-byte control-record schema. The surviving sibling `DomainRecord` has the identical guard (`src/domain/record.rs:105`, `d.array()? != Some(6)`), but `tests/domain_records.rs` covers truncation, overlong headers, trailing bytes, oversize and version bump — **not** indefinite-length. Also dropped: the embedded-NUL name case.

**Medium — preserved bootstrap assessment is not re-derivable.** `bootstrap-unpinned-v1/` lacks `artifact/` and `root/`, which its own `verify.py` dereferences. Disclosed honestly in `external-artifacts.json` (`"durably_archived": false`), but every export-graph claim is currently self-attested against `/var/tmp` state outside the repo.

**Low — README count drift.** README claims "192 rootless tests and 50 doctests"; `preservation/m7-review/policy-removal-static/summary.json` records `195`/`51`.

**Low — dead API and stale comments.** `UserNamespace::<Mapped>::idmap` and `NamespaceViewFailure` (`src/isolation/user.rs:166-188`) now have no callers. Stale policy references remain at `src/storage/volume.rs:173` and `src/filesystem/attachment.rs:275`.

### Receiver cleanup fix — correct

The fix hoists `gate.wait_ready_until` out of the `admitted` closure so the supervision channel is still held while diagnostics drain and `mount.close()` runs. Ordering is right: `running.wait_until` precedes `diagnostics.finish()`, so pipe writers are gone before the relay is joined — matching the project rule and the pre-existing arm's comment. `setup_deadline` is reused throughout; no renewal, no sleep. The `off-v1 → off-v2` delta confirms it fixed a real bug: v1 showed `Directory not empty (os error 39)` with the EPERM diagnostic lost; v2 shows the diagnostic preserved and `"runtime_retention_reported": false`.

Two notes, neither a defect: the asymmetry where the new arm calls `mount.close()` but the later `admitted` arm retains `Startup{mount}` is intentional — the caller (`src/execution/helper.rs:394-396`) stringifies immediately before guaranteed process exit, so retained owners there serve diagnostics, not recovery. On the inner `wait_until` failure path the relay is dropped unjoined (`DiagnosticRelay` has no `Drop`, `src/process/diagnostics.rs:15-18`), detaching threads — acceptable in an exit-only helper, but it is the one path that can lose a final message.

**Positive confirmations:** typestate seals intact (`mod private` still private, no `Default`/`From` minting a late state); no new minted deadlines (`grep '^+.*Instant::now'` over the range yields only a doc comment); failure types still own recovery resources; `pins.rs` removal orphaned nothing (every type it released no longer exists); bpffs removal symmetric in create and teardown; build-dependency removal propagated consistently through `Cargo.toml`, `build.rs` (deleted), CI and README; `policy/labels.bpf.c` retained as recoverable history. ARCHITECTURE.md matches source precisely, including the honest `namespace_for_root` rationale and the "anonymous inode allocation can still change filesystem metadata" caveat. A unit test for `AssignedRoots::try_from` rejection appeared in `labels.rs:80-93` during this review, closing part of the coverage gap.

### Scope and coverage limits

- **amd64/KVM only.** No arm64 execution, no TDX/SEV-SNP/CCA evidence.
- **No run exercises the full confinement boundary with the global exception active** — pivot_root, private proc, descriptor audit, seccomp, network isolation, descendant drain. The harness that asserted these is the unmigrated one.
- **`validation/rewrite-command-integration/` does not compile** against removed APIs. Per your instruction I treat this as an outstanding qualification obligation, not a new bug. Only `runtime.py` is still consumed (by `tests/distro/run.py`); the remaining ~1,900 lines of confinement assertions have no replacement in `tests/`.
- **Mechanism probes are not production qualification.** Every verdict says so (`full_distro_qualified: false`, `production_vm_validated: false`, `boot_passed: false`).
- I ran no cargo builds and no privileged operations; mechanical claims rest on your preserved logs plus my static reading.

I did not propose restoring policy registration anywhere.
