EVIDENCE CORRECTION (2026-10-21)
The original capability claim below is superseded by ../runtime-validation/.
The Bubblewrap command added capabilities without first dropping ALL; its actual
capability set therefore included CAP_SYS_ADMIN. The original metadata outcomes
remain historical evidence, but do NOT establish operation without SYS_ADMIN.
The newer explicit-allowlist probe preserves foreign labels; its child control
removes SYS_ADMIN and gets EPERM. This is capability authority in the filesystem
owning user namespace, not in the initial user namespace.

Two sibling sandboxes — validated prototype, 2026-10-21

Result
Root supervisor + independent filesystem-worker sandbox + command sandbox
successfully communicate through FUSE. Five lifecycle cases and their metadata
checks pass. No further fuse2fs/libfuse patch was needed beyond the previous
placement correction. This is topology feasibility, not production qualification.

Topology
The trusted root coordinator runs in a disposable Fedora7.2.10 x86 VM. A trusted
helper opens /dev/fuse and mounts with default_permissions in a namespace mapping
target IDs0..65535 to outer100000..165535. It passes the connection to the supervisor.
The worker launcher drops to outer UID/GID200000, creates its own user/mount/PID/
network/IPC/UTS namespaces, and drops every capability. The command sandbox uses
the helper's user namespace and separate mount/PID/network/IPC/UTS namespaces.
The final runner asserts that all six namespace identities differ among worker,
command and supervisor for each concurrent pair. The sandboxes are siblings,
not nested in each other.

Bubblewrap0.12.0 provides prototype setup, not a production runtime selection.
Worker receives its scoped read/write backing image, a FUSE connection FD, private
proc/dev/tmp and a read-only runtime tree. Commands receive the mounted FUSE
filesystem at /image, private proc/dev/tmp and read-only test tools. Worker private
storage is absent from the command filesystem. Supervisor/context sentinels are
absent from both, despite positive readability outside. Sandbox proc views contain
only their own identities. Each sandbox fails to connect to owned supervisor TCP
loopback and abstract Unix endpoints; both endpoints have positive controls.
Worker has zero effective capabilities and no_new_privs. A limited command
namespace capability set for ownership/credential/permission operations was
intended, but not enforced by dropping the inherited set. The original claim
that they lacked CAP_SYS_ADMIN was incorrect; see the correction above. Both
have disconnected private networking in this probe.

Bubblewrap has no arbitrary FD-preservation option in this version. The trusted
launcher transports the FUSE FD through stdin during setup; an in-sandbox helper
moves it to FD3 and replaces stdin with /dev/null before execing fuse2fs. Commands
receive no FUSE/control FD. The command's Python/ctypes runtime opens a read-only
libffi descriptor within its exposed runtime tree; this is explicitly allowed
and checked rather than misclassified as a leaked supervisor descriptor.
Worker output descriptors refer only to assigned diagnostic logs. Command output
uses a pipe. Neither receives the guest console as an input or output descriptor.

Cases and checks
- Normal: metadata operations, clean shutdown, fsck and independent remount.
- Command exit42: failure observed; orderly worker shutdown and remount check.
- Command sandbox launcher SIGKILL: descendants terminate, mount released, worker
  exits; clean filesystem independently checked.
- Worker sandbox launcher SIGKILL: command receives EIO/ENOTCONN/ECONNABORTED;
  mount and connection released; no clean-output acceptance attempted.
- Setup helper SIGKILL: command sandbox dies via the tested parent-death chain;
  its mount disappears and worker exits. No clean-output acceptance attempted.

All cases exercise UID/GID, ACL user/group allow-deny and inheritance, foreign
SELinux labels, file capabilities and corrected supplementary-group setgid.
The chmod matrix includes >32 groups, nonmember/nonowner controls and callers
with/without CAP_FSETID. Ambient and inheritable FSETID are explicitly removed
for the negative capability case, since bubblewrap propagates namespace caps.
Three clean shutdown cases pass independent ext4 checks and remount assertions.
At completion no matching sandbox processes or FUSE connections remain, and
supervisor/context sentinel bytes are unchanged.

Limits
No exploit or malicious filesystem was used. These are owned-object positive/
negative tests, not proof of absence of sandbox escape vulnerabilities.
Backing storage is a RAM-backed regular ext4 image, not the full device-mapper
carapace chain. This does not validate block-device ioctl restrictions.
Test tools run from a read-only runtime with /image as the working filesystem;
bootstrapping an actual distro root under this complete topology is separate.
No seccomp filter or resource-budget profile was composed in this fixture.
No network-enabled command case, supervisor-death recovery, concurrent builds,
production artifact publisher or confidential-platform hardware was tested.
FAILED_BUILD_NOT_PUBLISHED means the harness skips acceptance/inspection on
worker/helper death; there is no production publisher in this experiment.
The worker remains able to corrupt its assigned filesystem. Its sandbox limits
its authority; it does not establish correctness of a compromised worker's data.

Reproduce
 python3 validation/sandbox/two-sandboxes/run.py /var/tmp/NEW-two-sandboxes
Requires /var/tmp/conglobate-placement-fixed-v2, local bubblewrap, and the existing
guest packager dependencies. Host root/policy changes and host mounts are unused.
Guest has no host shares/disks/NIC; mutable images are on guest tmpfs.

Evidence
../reproduced/two-sandboxes contains the completed-v10 log, verdict, launch command,
input hashes and prior failed-run logs. Full fixture: /var/tmp/conglobate-two-
sandboxes-v10. Earlier failures were harness corrections: loopback setup, bwrap
option/capability spelling, permitted runtime FD, ambient-capability negative
control, supervisor ancestor excluded from final process scan, and accepting
the observed ECONNABORTED worker-death error. Failures are retained, not passes.
Production conglobate and installed worker/library binaries are unchanged.
