Resolver injection setup cancellation
=====================================

Reproduce:
  python3 validation/sandbox/guest.py resolver-lifecycle \
    --kernel-version 7.2.10-200.fc44.x86_64 --out /tmp/NEW-RESOLVER-LIFECYCLE --run

Disposable guest, no host disks/shares/NIC. Trusted setup runs in private mount
and PID namespaces with recursively private propagation. The source contains
captured resolver fixture bytes and the image target has distinct original
bytes. Both paths are known regular files. A gate before bind, after bind, or
after read-only remount hands the trusted namespace-init PID to the controller.
The controller pins a pidfd and kills that init while it waits at the gate.

All three cases pass:
- parent mountinfo is unchanged before and after cancellation;
- source and target device/inode, UID/GID, mode and bytes are unchanged;
- the parent never observes injected bytes while the child bind is live;
- launcher exits, namespace init disappears, and a /proc scan finds no process
  retaining the failed setup's mount namespace;
- at the final gate, the setup saw the captured resolver bytes and attempted
  writing through the mount failed read-only.
The source's inode/content is never replaced. Each handoff and reap has a
five-second controller timeout; the child gate is finite. No untrusted command
is launched before or during these trusted setup gates.

Scope: ordinary file-backed synthetic image path in trusted initial-root setup.
This is not the production resolver implementation (none exists yet), arbitrary
path resolution, user-namespace/FUSE worker integration, filesystem-write
rollback, process-death durability, or cancellation during a blocked mount
syscall. Known regular/symlink live command behavior is separately covered by
network-policy. This fixture does not change any runtime or DNS contract.

Evidence: reproduced/resolver-lifecycle (source snapshots, kernel/tool hashes,
command and successful guest result).
