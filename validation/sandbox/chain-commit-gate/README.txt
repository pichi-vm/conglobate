Outer publication gate prototype
================================
Reproduce cases chain-commit-gate and chain-commit-gate-failure:
  python3 validation/sandbox/guest.py CASE --kernel-version 7.2.10-200.fc44.x86_64 --worker-build /tmp/conglobate-finalize-workers --out /tmp/NEW-commit-gate --run

The actual fixed build helper still creates its complete verified OCI artifact
and inventory at /work/published. In this fixture that directory is INTERNAL
staging; /work/result is the public destination. The recipe root does not expose
these paths. All output remains on the disposable guest's tmpfs.

The outer publisher waits for the dedicated launcher's status, which reflects
PID init's final status after reaping and checking descendants. The init shell
requires no DM/loop residue before invoking commit-gate.py. Only owner status 0
permits the safe-Rust commit-output helper to rename staged output to result
with RENAME_NOREPLACE, then sync the parent directory. File hashes are checked
before/after the successful rename. Failure mode injects status 17 in trusted
init after actual successful artifact creation and descendant checks; the public
destination must remain absent and internal staged bytes unchanged.

This demonstrates placement of a completion gate, not a final publication
protocol or a real filesystem cleanup-error injection. It does not prove safe
startup, cancellation/commit arbitration, owner death during transfer/commit,
post-commit acknowledgment, cross-filesystem transfer, orphan staging recovery,
concurrent collisions or persistent-storage power-loss durability. Internal
staging remains after the failed attempt intentionally, for inspection. Existing
helper publication log markers refer to INTERNAL staging in these two fixtures,
not the public result. Safe Rust covers the final rename; trusted Python/shell
still coordinates fixture processes. No production publication API is changed.

Results: both controls pass. Complete artifact/internal inventory creation and
storage cleanup precede the outer gate. Success renames to the public destination
with identical file hashes; injected final init status 17 leaves no public output.
Retained evidence: reproduced/chain-commit-gate and chain-commit-gate-failure.
The /work tmpfs is owned outside the build lifetime namespace, so completed staged
bytes survive that namespace's exit without keeping a build actor alive.

Live-descendant completion rejection
-------------------------------------
Case chain-commit-gate-descendant starts a harmless 30-second sleep process from
trusted init after actual artifact staging. An outer observer verifies its host
and namespace PIDs, opens its pidfd and confirms it is alive before releasing
the completion check. The existing check must reject the non-zombie child.
The launcher must return failure, its namespace must terminate that child
(observed through the pinned pidfd), and the outer gate must leave public output
absent. No explicit signal is sent to the child on the passing path. Device and
loop cleanup is checked before the gate. This exercises a real failed liveness
check rather than injecting status 17; it does not simulate escaping a command
namespace or establish coverage of every kind of surviving descendant.

Result: live child is recorded as sleeping, completion assertion rejects it,
launcher returns 1, pinned pidfd confirms termination, storage checks pass, and
public output remains absent. Internal artifact remains for inspection. Evidence:
reproduced/chain-commit-gate-descendant. No passing-path observer child kill.

Publisher death, retry and destination collision
------------------------------------------------
Case chain-commit-death runs one real successful build through init/resource
completion, then copies that completed internal artifact into isolated test
staging directories. The safe-Rust commit helper has diagnostic stop gates
before rename, after rename and after parent sync. The outer test pins its pidfd,
verifies the stop and kills only the committer. No build commands are active.

Before rename: no public destination, staged files preserved, and retry may
commit. After rename or sync: complete public bytes remain without a returned
success marker; a re-staged retry must fail EEXIST without replacing the output
inode or changing its bytes. A separate collision case creates a real destination
while publisher is stopped before rename; resuming must fail EEXIST and preserve
both that destination and staging. Every case checks file hashes, and the original
completed artifact must remain unchanged. All per-case directories are removed
only after assertions; failure leaves diagnostic state in the disposable guest.

The gates are validation-only. This is tmpfs process-death behavior, not storage
power-loss durability. It demonstrates unacknowledged committed output and safe
non-overwrite behavior, not automatic retry reconciliation or staging garbage
collection. No cancellation acceptance protocol is implemented by these gates.

Result: all three death/retry gates and concurrent-destination control pass.
Evidence: reproduced/chain-commit-death. Completed build, init exit and resource
checks precede the matrix. A new destination inode appears only at rename; later
process death prevents the return marker but does not roll back that output.
