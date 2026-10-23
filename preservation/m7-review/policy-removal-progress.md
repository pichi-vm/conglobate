# Registration-free lifecycle implementation in progress

Base: 522011a; uncommitted migration, not qualified for execution.

- Step and preparation now retain namespace ownership directly, with no
  installation/activation/disable/close or policy retry phases.
- Storage handoff takes the namespace handle; assigned volume views still
  originate from the owned sealed storage layout.
- Runtime no longer mounts private bpffs. BPF build script and libbpf
  dependencies are removed; Cargo.lock updated by cargo.
- Policy source, tests, filesystem pin and related reference types are retained
  in preservation/scoped-policy-522011a with archive hashes and verified original
  bytes. The old policy/ asset is still present pending fixture migration.
- Restricted trusted leaf receives AssignedRoots from VolumeInput, through
  internal CBOR helper arguments. It performs anonymous per-root descriptor
  label set/read/remove before readiness. Bootstrap probes both roots.
- Namespace pinning still exists and needs evaluation/removal where its only
  purpose was policy identity retention; do not call the simplification complete.
- Old policy fixture APIs must migrate to external policy loading. No VM test
  of the new production lifecycle has run. Existing VM passes do not qualify it.

Rootless lib/tests passed after initial lifecycle/probe changes; subsequent
filesystem-pin deletion required restoring the independent RecoveryDirectory
import. Strict Clippy then identified a single-element runtime mount loop, now
removed. Logs are in /var/tmp/conglobate-policy-removal-*.log and must be retained
with final validation. All-target compilation passed before the pin cleanup.
Next: finish static checks, repair stale doctest/comment claims, migrate VM
harness and test absent-policy rejection plus successful single/dual-root runs.
M4/M5/M6 remain at zero adherence passes. Goal remains active.

## First production-path VM runs

Maintained distro environment now compiles/loads the fixed exception externally;
the binary has no BPF dependency. Strict Clippy and 51 doctests pass. Rootless
lib/tests passed 195 tests. Archived old policy tests explain the reduced count.

- label-admission-off-v1: recipe sentinel absent and build rejected, but diagnostic
  says joining peer closed and runtime cleanup reports Directory not empty.
  Not a clean negative pass. Need diagnostic-before-gate-loss ordering and inspect
  failed helper attachment cleanup before declaring error-path correctness.
- label-admission-on-v1: command sentinel and build/export success marker observed;
  exported OCI extracted separately and all blob SHA-256 identities verified.
  Runner itself reports failure because kernel sysrq output interleaved/truncated
  the final DISTRO_BUILD_EXIT line. Original verdict retained unchanged;
  supplemental evidence is separate, not a fabricated passing runner result.

No production-code commit yet. Full VM suite migration, anonymous probe with
bootstrap both roots, namespace-pin simplification and failure regressions remain.


The off-v2 rerun now preserves the restricted labeling EPERM diagnostic,
reports cleanup complete, emits no command sentinel and reports no runtime
retention/cleanup error. Result-device record is exit 1. Separate negative
assessment preserves the original build-failed verdict. The receiver now keeps
its supervisor channel alive while waiting/reaping failed restricted setup,
draining diagnostics and closing the view. This fixes the channel-drop race
without sleeps. Positive on-v2 is running with the same source.
The harness writes status to an isolated second virtio device and syncs before
reboot; stdout is no longer the authority for the final exit code.


on-v2 completed successfully: command marker, publication/export, separate
exit-0 record and every blob hash passed. on-v2 and off-v2 are complementary
native amd64 single-root production admission evidence. They do not replace
the broader old VM suite, bootstrap checks or platform qualification. Rootless
195-test rerun and strict Clippy pass after the failure-path correction. The
reopened M4/M5/M6 gates remain at zero. Namespace pin removal and VM fixture
API migration remain concrete next work, not claimed complete by this commit.
