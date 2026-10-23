All findings now have corresponding work. Updating dispositions.

## Updated dispositions — findings addressed since my report

The tree advanced six commits past the reviewed range (`c1073b9..a3c528a`) while my reviewers were running. Each of my findings now has corresponding work. Verified against source, not commit messages:

**High — bootstrap second-root probe unfalsified → ADDRESSED.** `tests/distro/label-operations/recipe.yaml` loops `for root in / /mnt/` and, on **each** root, writes a label, reads it back with exact value comparison, asserts `security.selinux.extra`/`security.other`/`trusted.conglobate` are all rejected, then removes and re-sets across a remount. `label-operations-v1` records both markers passing. This falsifies the conditional arm: deleting `if let Self::Bootstrap` would now fail the `/mnt` iteration. Note the assessment correctly keeps `"differential_admission_probe_qualified": false` — the test proves command-side labeling works on both roots, not that the *trusted probe* ran on both. That residual distinction is honestly retained.

**High — no command-side labeling assertion under new code → ADDRESSED.** The same recipe asserts from the untrusted command's perspective, including the read-only negative (`setfattr` on `/sys` must fail with `Read-only file system`) and attribute-exclusion negatives. This supersedes my observation that `fedora-build-v3` predated the migration.

**Medium — orphaned CBOR invariant → FIXED.** `a3c528a` adds the indefinite-length rejection (`vec![0x9f]` + `0xff`) and the embedded-NUL name (`"conglobate-command-a\0"`) to `tests/domain_records.rs`, restoring both invariants I identified as orphaned by the `tests/namespace_records.rs` deletion.

**Low — `labels.rs` untested → PARTIALLY FIXED.** `rejects_unknown_root_layouts` now exercises `AssignedRoots::try_from` across `2..=u8::MAX`. The probe's I/O paths remain VM-only, which is appropriate.

**Low — no automated negative acceptance criterion → FIXED.** `tests/distro/labeling/verify_missing.py` replaces hand-authored negative assessments with five programmatic checks, including `status.raw` (the unforgeable result device) and `'runtime cleanup:' not in serial`.

## Two new source changes in scope

**`09d698c` — receiver mountpoint lifetime.** `MountPoint::create` (which made a `tempfile::TempDir` under a tmpfs-verified parent) is replaced by `MountPoint::open`, which holds `(parent: File, name: OsString)` for an **existing supervisor-owned** directory. This is the correct direction: the helper no longer creates shared state that can outlive its cancellation, and the dropped tmpfs-magic check moves the trust to the supervisor that owns the directory. It also eliminates the `Directory not empty` class of failure at its root rather than patching cleanup. Consistent with the RAII/ownership preferences.

**`256e2c4` → `cli-timeout-v2`.** This is the pattern I most want to see: a test authored to *expose* a failure (`"runtime_cleanup_complete": false`, `"passed": false` in v1), the preserved failing evidence retained, then the fix and a passing v2. The timeout path now completes runtime cleanup. `346d793` (named pipes allowed, device-node restrictions retained) and `bfc0ba4` (confinement checks migrated to the production CLI) begin closing the unmigrated-harness gap I flagged.

## What still stands

- **Medium — `labels.rs` cannot self-evidence two distinct filesystems.** Unchanged. The `st_dev` inequality check between the `/` and `/mnt` handles is still absent, and `label-operations` tests the *command's* access, not the trusted probe's target selection. `"differential_admission_probe_qualified": false` is the right label for that gap.
- **Medium — probe error-context asymmetry.** Unchanged at `src/isolation/labels.rs:69-75`; `fgetxattr`/`fremovexattr` failures still surface as bare errnos with no indication of which root failed.
- **Medium — `bootstrap-unpinned-v1` not re-derivable** (`"durably_archived": false`). Unchanged.
- **Low — README count drift** (192/50 vs. preserved 195/51); now certainly stale given the added tests.
- **Low — dead `UserNamespace::<Mapped>::idmap`, stale policy comments** at `src/storage/volume.rs:173` and `src/filesystem/attachment.rs:275`.

## Scope limits — unchanged

Still amd64/KVM only; no arm64, TDX/SEV-SNP/CCA evidence. `validation/rewrite-command-integration/` remains unmigrated, though `bfc0ba4` has begun moving its confinement assertions to the production CLI. Every new assessment correctly carries `full_sandbox_qualified: false`. No milestone gate is claimed passed, and I have made no edits, run no privileged operations, and proposed no restoration of policy registration.
