# M4 lifecycle reconciliation in progress

Scope: 416ca11. M2/M3 gates are restored; M4 remains 0/3. This is a partial
source/evidence reconciliation, not a complete milestone adherence pass.

Reviewed Step/ExecutionStorage, preparation and finalization, command domains,
Runtime and Volume owners against the linear workflow and environment-owned ID
ranges. CommandSandbox retains immutable range values and derives transferred
mount views from its own storage. Single-threaded helper entry joins the supplied
network before the user namespace and drops those descriptors. Capabilities and
syscall restriction remain separate prerequisites to recipe admission.

CommandOutcome remains independent of helper exit and cleanup. Observation fixes
the teardown deadline once. Cancellation, helper reap and descendant drain precede
unmount/validation. Finalization retry retains the original cutoff and diagnostics;
StorageExecutionFailure can recover rejected storage, never ExecutedStorage.
Runtime failure cannot return successful stage output. Drop is fallback only;
unfinished steps retain mapped namespace/storage until disposal. Scope checks
compare the acquisition namespace, not PID 1. No serialized cgroup record exists.

Current validation batch exercises maintained production-library fixtures in
native amd64 disposable VMs only. The supervisor-abort fixture is the linked
library supervisor, not a full CLI publication/crash test. Writeback exhaustion
and blocked I/O affect only fixture-owned devices inside the guest. These results
must not be described as complete command-sandbox or platform qualification.

Remaining work before a complete M4 gate includes whole-command confinement and
labeling evidence reconciliation, enabled/offline networking composition, all
historical finding dispositions against changed source, and review of unqualified
failure branches. Native results cannot qualify Arm or confidential hardware.
M5/M6/M7/M8 advancement is not authorized by this partial review.

The 19-case batch passed. Each source archive was compared byte-for-byte with
current production src/, Cargo.toml and Cargo.lock; source-check.json records
hashes per case. The retained assessment.json files require exact markers, status
records and poweroff. Supervisor-death deliberately expects status 134; it is not
misreported as an exit-zero supervisor. All results keep full_lifecycle_qualified
false. Large initramfs/kernel bytes remain local with hashes; M8 durability is
not established. Whole-command confinement is being refreshed separately.

Whole CLI confinement, enabled/offline DNS/resolver composition, and two-root
foreign-label command operations also pass at 416ca11. Confinement checks seeded
supervisor keys, command descriptors/capabilities/proc visibility, fresh runtime
mounts and live-descendant cleanup. Network checks enabled namespace reuse,
read-only injection and restoration of the image's original symlink/content.
Label commands set/read/remove/restore foreign labels on both roots across remount,
reject unrelated protected attributes and reject writes to read-only /sys.

The confinement first invocation used a mistyped pin and was correctly rejected;
its failure is preserved separately and was fixed only by correcting the launcher
argument. No production patch was made. Canonical inventory and one produced
carapace scute were additionally verified for confinement/network outputs.
Rootless CI 40046122488 is green at 416ca11; hosted lifecycle CI remains pending
at this observation. Claude's focused M4 review is running separately; findings
must be source-checked and discussed before any milestone gate is advanced.
