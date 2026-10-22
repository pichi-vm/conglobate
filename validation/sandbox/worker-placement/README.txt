FUSE worker outside the command user namespace — 2026-10-21

Follow-up: the reproduced chmod defect is corrected and regression-tested in
../worker-placement-fix/README.txt. The original failure below is retained as
historical evidence; the corrected profile requires kernel permission checking.

Verdict: descriptor handoff, core metadata and bounded lifecycle checks work.
The existing experimental worker/library pair is NOT fully compatible with
this placement: supplementary-group chmod silently loses the setgid bit.
Do not mark the placement accepted until that behavior is corrected.

Reproduce (requires the retained experimental worker build):
  python3 validation/sandbox/worker-placement/run.py /var/tmp/NEW-placement-run

Topology
Disposable Fedora 7.2.10-200.fc44.x86_64 guest, SELinux disabled in guest;
host policy unchanged. No guest NIC, host shares or physical disks. Mutable
test images live in guest tmpfs. Trusted helper uses a user/mount namespace,
mapping 0..65535 to 100000..165535. It opens /dev/fuse, mounts in that namespace,
and passes the descriptor through SCM_RIGHTS. libfuse's existing /dev/fd/N
mountpoint interface accepts the established connection without modification.
Worker runs as host UID/GID200000, no supplementary groups, zero capabilities,
no_new_privs, in the guest's initial user namespace. Only it owns the mode0600
backing file. Commands have target UID0 and can use mapped target IDs.
Control worker runs in the SAME user namespace as the client, host UID100000.
Both use the earlier ACL/protocol-patched fuse2fs/libfuse pair, not distro stock
fuse2fs. No new worker/library patch was made for this test.

Observed
- Worker and client namespace inode identities differ; worker credentials and
  zero effective capabilities confirmed through /proc by trusted coordinator.
- UID1234/GID2345 ownership, foreign security.selinux and exact revision2 file
  capability bytes survive independent read-only kernel remount after clean
  unmount. All four clean images pass e2fsck -fn.
- Named-user ACL allow and deny controls, supplementary-group ACL access and
  default ACL inheritance pass through FUSE with kernel permission checking.
- Command namespace-root cannot signal-query the owned worker, open its /proc
  descriptor directory or PTRACE_SEIZE it. These are explicit EPERM/EACCES
  assertions against the fixture-owned process, not an escape exploit.
- Normal completion, command exit42, command child SIGKILL, helper SIGKILL and
  worker SIGKILL all terminate/reap the owned processes and release every FUSE
  connection. Killing the worker makes subsequent filesystem I/O fail.
  Same-userns control is the sixth lifecycle case. Worker-death output is not
  validated as clean or accepted; no artifact publication is attempted.

Confirmed limitation and source explanation
Owned file: UID1234, GID3456. Caller: UID1234, primary GID1234, supplementary
GID3456. chmod(02750) returns success. Same-userns control stores02750; outside
worker stores0750. The change persists on independent remount.

fuse2fs op_chmod calls in_file_group, which compares inode_gid (target3456)
with fuse_getgroups output. libfuse fuse_req_getgroups reads Groups from
/proc/<request-pid>/task/<request-pid>/status without translating the returned
group numbers into the connection user namespace. The outside worker reads
host-mapped IDs rather than target IDs. Kernel ACL checks do not fix this
separate worker-side chmod decision. The correction must preserve credential
and race semantics; simply dropping group checks is not an acceptable fix.

Scope
This isolates namespace placement, not the complete production sandbox.
Clients deliberately retain the guest's outer /proc view to test direct
permission denial even when the owned worker PID is visible. Private command
PID/proc/root mounts and final syscall policy are NOT integrated here.
The command-death case reaps one bounded child, not a descendant stress test.
Helper-death cleanup depends on it holding the sole mount namespace reference;
additional command/mount holders require the existing lifetime owner design.
Tests cover x86 in an ordinary VM, not three confidential platforms.
This outcome is a worker/library identity-translation problem, separate from
kernel restrictions on trusted xattrs, device creation and immutable flags.

Evidence
../reproduced/worker-placement contains the initial assertion failure, final
completed matrix, input hashes, exact launch command, relevant source snapshots
and worker-build identity. Completed run: /var/tmp/conglobate-worker-placement-v4.
An intermediate packaging run in /tmp hit a disk quota before boot; subsequent
runs used /var/tmp. Final guest exited0 after explicit expected-limit assertions.
PLACEMENT_MATRIX_COMPLETE means observations completed, not full compatibility.
