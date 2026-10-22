Outside-userns worker chmod correction — 2026-10-21

Result: patched and validated in the isolated experimental backend.
No production conglobate source, installed worker or installed library changed.

Change
fuse2fs op_chmod no longer repeats the supplementary-group/setgid check when
ff->kernel is enabled. This mode selects default_permissions; Linux FUSE calls
setattr_prepare before dispatching SETATTR. That check enforces ownership and
removes S_ISGID if the caller lacks group membership or the relevant capability.
The worker must preserve the resulting mode instead of attempting a second
decision through /proc with different user-namespace numbering.

The non-kernel permission path retains its original checks. This is NOT a
general repair to fuse_getgroups or its /proc identity/race semantics. No new
libfuse patch is needed; the existing experimental ACL library is byte-identical.
For a pre-mounted descriptor, the trusted mount helper MUST actually enable
default_permissions. Passing -o kernel to a daemon cannot retroactively set
mount options on an already-created connection. The fixture explicitly sets
default_permissions at mount time. This is part of the production contract.

Build and reproduce
 python3 validation/sandbox/worker-placement-fix/build.py \
   --pair /tmp/conglobate-acl-protocol-v4 --out /var/tmp/NEW-worker
 python3 validation/sandbox/worker-placement/run.py \
   /var/tmp/NEW-placement /var/tmp/NEW-worker --fixed
 python3 validation/sandbox/guest.py acl-protocol \
   --kernel-version 7.2.10-200.fc44.x86_64 \
   --worker-build /var/tmp/NEW-worker --out /var/tmp/NEW-acl --run

The builder copies the original source and performs a fresh configure/build.
Compiler scratch uses the new build directory because /tmp hit its user quota
on the first attempt. That configure failure preceded any guest execution.

Validation
1. chmod decision matrix matches native kernel ext4 and same-userns control:
   primary-group membership, supplementary membership, >32 supplementary groups,
   nonmember stripping, and nonowner rejection. Nonroot CAP_FSETID preserves
   setgid; root without CAP_FSETID outside the file group loses it. Actual
   effective capability states are asserted in the test children.
2. Corrected behavior passes with a UID200000, zero-capability worker outside
   the client's user namespace. Ownership, ACL access/inheritance, file capability
   bytes and foreign SELinux labels still pass, with independent kernel remounts.
3. Worker signal-query, /proc FD-directory and ptrace denials remain effective.
4. Six lifecycle cases complete: same-userns control, normal, command exit42,
   command child death, helper death and worker death. Connections are released;
   four clean images pass e2fsck and independent remount. Worker-death output is
   not accepted or asserted clean.
5. Existing ACL protocol regression passes kernel/initial-FUSE/mapped-FUSE with
   and without metadata_csum (six configurations), including capability-aware
   ACL/setgid cases and exact on-disk ACL/mode comparisons.

Evidence: ../reproduced/worker-placement-fix/{placement,acl}, patch, build logs,
source evidence and manifests. Complete runs are retained at:
 /var/tmp/conglobate-placement-fixed-v2
 /var/tmp/conglobate-placement-fixed-test-v1
 /var/tmp/conglobate-placement-fixed-acl-v1

Limits
This closes the reproduced placement-specific chmod defect, not every possible
worker/library namespace assumption. The scope limits in worker-placement remain:
ordinary x86 VM, deliberately visible outer /proc for permission-denial tests,
no integrated private command PID/root layout or production enforcement policy.
Kernel trusted-xattr/device/immutable restrictions are unchanged. Existing
worker patches still need maintained adoption; birth-time patch was not included
in this pair. No upstream submission or full confidential-platform validation.
