Candidate command restrictions — bounded validation
==================================================

This is a diagnostic policy, not the production executor or a security verdict.
The VM kernel, supervisor and filesystem worker remain trusted. Only recipe
commands receive this policy. All resources and canaries exist in disposable
guests without host shares, host disks or NICs.

Reproduce the small fixture:
  python3 validation/sandbox/guest.py policy \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --out /tmp/NEW-POLICY-FIXTURE --run

Reproduce the larger Fedora/FUSE workload using the retained local seed:
  python3 validation/sandbox/policy/package.py \
    --seed /tmp/conglobate-fuse-adversarial \
    --out /tmp/NEW-PACKAGE-POLICY-FIXTURE

The historical seed can now be replaced by the repository-based builder in
../package-fixture/README.txt. Inputs.json hashes every packaged regular file, including RPMs,
kernel and entry helper. Symlinks and the exact QEMU command are also recorded.
Known ACL failures are diagnostic outcomes, not passing ACL assertions.

Policy applied by entry/helper, before image-controlled execution:
- Private namespaces and runtime mounts are established by the trusted launcher.
- Pivot root, detach old root, close descriptors >=3, clear command environment.
- Bounding/effective/permitted capabilities: CHOWN, DAC_OVERRIDE, FOWNER, FSETID,
  KILL, SETGID, SETUID, SETPCAP, NET_BIND_SERVICE, SYS_CHROOT, SYS_ADMIN, SETFCAP.
  These capabilities exist only in the owning user namespace. Inheritable and
  ambient capability sets are empty. no_new_privs is set.
- Deny mount APIs, unshare/setns, namespace-creating clone flags, tracing,
  process_vm, pidfd_getfd, keyring APIs, bpf/perf/userfaultfd/io_uring_setup,
  module/kexec/reboot/swap APIs. Ordinary process/thread creation remains usable.
- clone3 returns ENOSYS to allow libc fallback. Native x86_64 only: the explicit
  x32-number gate returns EPERM; the library rejects other audit architectures.
- socket permits AF_UNIX, AF_INET, AF_INET6 and AF_NETLINK only; AF_NETLINK
  permits NETLINK_ROUTE only. Non-netlink SOCK_RAW is denied, including type
  flag variants. socketpair permits AF_UNIX only. Rejected calls return EPERM.
  These restrictions apply to both networking modes; routing in the per-step
  namespace determines whether ordinary IP can reach upstream destinations.

Important findings:
1. The first policy omitted SYS_ADMIN. The FUSE workload failed at setting
   security.selinux with EPERM even with SELinux disabled in the guest. Restoring
   SYS_ADMIN inside the user namespace restores labeling. Retained failure:
   reproduced/policy-without-sys-admin/. It is NOT evidence that initial-namespace
   root is required. It does increase the importance of completing the remaining
   syscall/ioctl and reachable-object review before selecting this policy.
2. seccompiler 0.5.0 checks audit architecture, but x32 shares the x86_64 audit
   architecture. A native-syscall denylist needs an additional x32-number gate.
   The helper adds three safe BPF instructions; a harmless x32 getpid request
   returns EPERM in the guest. Source: seccompiler src/backend/bpf.rs and
   src/backend/filter.rs. Rejecting x32 is a compatibility limitation.
3. Capability reduction must reduce effective before permitted capabilities:
   the effective set must remain a subset of permitted. The initial fixture
   caught an incorrect order with capset EPERM before any image command ran.

Observed passing evidence:
- Expected capability sets, no_new_privs and installed seccomp at command entry.
- Closed inherited files (FD9/257), TCP and pathname Unix listeners (258/259),
  and a synthetic pipe (270); outside paths absent; command environment reset.
- Null-argument denied-syscall assertions and namespace-creating clone requests.
  These check policy dispatch, not exploits. Raw IPv4/packet sockets denied.
- Two consecutive independently namespaced steps pass the same assertions.
- Private devpts supports openpty and terminal-size ioctls. /sys is absent,
  /dev contains only fixture null/ptmx/pts. NS_GET_PARENT on the command's own
  user-namespace handle is denied. This is not a complete ioctl allowlist audit.
- Ordinary ownership/mode/link operations, shell fork and Python threads work.
- Larger FUSE run retains SELinux/capability metadata, filesystem smoke checks,
  PID invisibility, abstract-socket isolation and detached-descendant teardown.
  New proc mounts are denied by the policy rather than permitted in this run.
- RPM reinstall, erase and install of the staged attr package succeed INSIDE
  the restricted command. Initial full Fedora installation still runs before
  restrictions. This does not qualify a full OS/kernel/initramfs build or a
  version-changing package upgrade.

Evidence:
  reproduced/policy/{inputs.json,command.json,results.txt}
  reproduced/package-policy/{inputs.json,command.json,results.txt}
  reproduced/policy-without-sys-admin/{inputs.json,command.json,results.txt}
  reproduced/socket-baseline/{inputs.json,command.json,results.txt}
  reproduced/socket-policy/{inputs.json,command.json,results.txt}
  reproduced/package-sockets/{inputs.json,command.json,results.txt}

Socket-policy follow-up:
  The prior candidate allowed creation of AF_ALG and netlink usersock/audit/
  generic/uevent sockets. Creation alone is not evidence of a breakout or of
  permission to perform privileged operations. VSOCK was unavailable in the
  guest; absence of an installed transport is not policy enforcement.

  The narrowed policy passes in two successive command namespaces: ordinary
  Unix/IP/route socket creation; Unix stream/datagram/seqpacket socketpair
  exchanges; an isolated RTM_GETLINK query; IPv4/IPv6 disabled-routing errors;
  explicit EPERM for all other family numbers 0..63, selected non-route netlink
  protocols, optional VSOCK/ALG, non-Unix socketpair and flagged raw sockets.
  High register bits do not bypass the C-int family/protocol comparisons.
  No probe connects to an external endpoint or sends control messages to
  unrelated kernel interfaces. The guest has no external network device.

  The larger Fedora/FUSE fixture was rerun with this helper. Restricted attr
  RPM reinstall/remove/install, metadata, process teardown and worker-failure
  checks still pass. Its known ACL failures remain. This fixture installs the
  initial base before restrictions and does not prove all package workloads
  tolerate the narrower family/protocol policy.

  Enabled NAT/DNS under this revised helper now has a bounded integration run
  in ../network-policy/README.txt. General ioctl review and final production
  integration remain open. NETLINK_ROUTE still requires permission and
  namespace analysis for privileged operations; this test sends only a read-only
  link query. Inherited socket FDs are handled by the separate entry-close gate.

  Primary mechanism references:
  https://man7.org/linux/man-pages/man7/network_namespaces.7.html
  https://man7.org/linux/man-pages/man7/vsock.7.html
  https://man7.org/linux/man-pages/man7/netlink.7.html
  Network namespaces isolate the documented IP/abstract-Unix resources. VSOCK
  is a separate VM communication family independent of VM IP configuration;
  explicit rejection avoids relying on an absent transport in the appliance.

Safe integration and licensing:
  Entry helper: #![forbid(unsafe_code)], nix 0.30.1, caps 0.5.6,
  seccompiler 0.5.0. Dependency sources internally use platform unsafe code;
  the conglobate-facing interfaces and helper do not require unsafe blocks.
  Cargo metadata: caps MIT/Apache-2.0; seccompiler Apache-2.0 OR BSD-3-Clause.
  This establishes available license choices, not a full dependency audit.
  https://docs.rs/seccompiler/0.5.0/seccompiler/

Open boundaries:
  This is a denylist experiment, not an exhaustive kernel interface policy.
  No claims about unsupported ABIs, all ioctl families, every namespace-handle
  route, loader poisoning before the trusted helper starts, full package builds,
  or the production executor. The trusted supervisor must sanitize the helper's
  OWN launch environment; clearing only the later image environment cannot
  protect a dynamically linked helper from a poisoned launch environment.
  Existing ACL and final-flush blockers remain unchanged.

Inherited IPC / pathname follow-up
---------------------------------
Run the ordinary policy case to reproduce the expanded driver/probe. Retained
run: ../reproduced/ipc-policy. The trusted driver creates a memfd, an ordinary
/dev/shm backing file, an eventfd and a connected Unix socketpair, and passes
them at FD271..274 in addition to the prior listener/pipe/file handles.
A namespace-only child (user/mount/PID/net/IPC/UTS, before pivot/FD closure)
reads the synthetic memory/file markers, consumes/restores the event counter,
and exchanges one fixed message over the socketpair. It can also connect to
a deliberately accessible pathname socket. None are real services or secrets.

The supervisor's abstract Unix listener has a successful same-namespace
positive control, but connection from the private network namespace fails
with ECONNREFUSED. This distinguishes abstract-name isolation from pathname
and descriptor access. The corrected restricted launcher then runs two steps:
all inherited descriptor numbers return EBADF, old socket/shm paths are absent,
and the prior capability/syscall/socket/PTY tests still pass. Supervisor memory
bytes, event value and bidirectional socketpair remain usable after both steps.

This uses a shared-memory backing file via open/pread, not shm_open/mmap or
cross-process futex qualification. Existing SysV tests are separate. It does
not close the per-UID/worker keyring, cross-build identity-reuse or every IPC
interface gates. Descriptor closure applies to numbers >=3; trusted stdio
construction remains a separate invariant.

Primary references checked 2026-10-20:
  https://man7.org/linux/man-pages/man7/network_namespaces.7.html
  https://man7.org/linux/man-pages/man7/unix.7.html
Network namespaces isolate abstract Unix names. Pathname sockets follow
filesystem reachability/permissions; Unix sockets also support descriptor
passing. Namespace creation does not replace closing inherited handles.

Runtime mount / proc control follow-up
--------------------------------------
Evidence: ../reproduced/runtime-mounts, including automatic local source
snapshots. The fixture now mounts proc nosuid,nodev,noexec and devpts
nosuid,noexec. Both command runs inspect /proc/self/mountinfo and require those
flags plus nosuid,noexec on /dev. The existing private terminal size ioctls,
/dev/null and candidate policy tests remain functional.

Each step tries O_WRONLY opens of /proc/sysrq-trigger, kernel/core_pattern,
kernel/modprobe and vm/drop_caches. All fail with EACCES/EPERM/EROFS; there are
no writes or ioctl calls on those controls. The files must exist: ENOENT is
not accepted. All actions are in the disposable guest. The readonly-open
namespace handle test remains limited to the command's own user namespace.

A disposable /dev/step-canary is absent at entry then created in each step;
the second step must not inherit it. After both steps the supervisor's entire
mountinfo and the image's underlying /dev listing match their starting state.
This establishes bounded sequential teardown/nonpropagation, not cancellation
or arbitrary device/ioctl isolation. /sys is absent in this fixture; it does
not qualify a sysfs view. Proc information exposure, the final masked-path
list and additional device/ioctl/workload requirements remain open. These are
prototype launcher changes only; production executor behavior is unchanged.

Corrected worker in the Fedora package fixture
---------------------------------------------
policy/package.py accepts --protocol-worker-build /tmp/ACL-PROTOCOL-PAIR with
the --seed and optional --upgrade-rpms arguments documented by package-fixture
and package-upgrade. Reproduction used both the pinned package seed and upgrade
RPMs. It installs the paired worker at a distinct /paired-worker path and its
matched libfuse library, and selects that worker for the package filesystem.

In this mode ACL child access statuses are assertions, not diagnostic output.
The unrelated user must be denied, the named user granted, and the created
child must inherit the named entry and expected mask. Those assertions pass,
as do initial DNF installation, restricted attr reinstall/remove/install,
version-changing config/scriptlet upgrade, kernel-remount output checks,
unchanged origin and existing command-isolation/descendant checks.

Evidence: ../reproduced/paired-package, including seed/input catalogs, source
identities and selected worker build hashes. The main worker/library bytes were
checked against the corrected pair. The fixture's separate crash-worker still
uses stock fuse2fs; its failure markers are not claimed as corrected-worker
fault qualification. The package runner now also requires one exact successful
guest-exit record.

Initial installation precedes the restricted command launcher. This is broader
worker/package compatibility evidence, not a full restricted bootstrap, whole
OS build, distribution upgrade, kernel/initramfs generation or performance
comparison. Those gates remain open.

Follow-up: --restricted-bootstrap also moves the initial DNF installation into
the restricted launcher. It passes with the corrected worker/library pair; see
../restricted-bootstrap/README.txt for exact scope, reproduction and evidence.

Valid-argument process access follow-up
--------------------------------------
Run the ordinary policy fixture to reproduce. Evidence is retained in
../reproduced/process-valid-arguments, including source snapshots and input
hashes. The unrestricted process-probe succeeds at process_vm_readv/writev
against its own fresh buffers, pidfd_getfd against its own canary memfd, and
PTRACE_TRACEME in a fresh owned child. The same valid calls return EPERM in each
of two restricted command namespaces; scratch buffers and memfd contents remain
unchanged. Thus these results supplement the older null-argument dispatch tests.

A second probe allocates a dedicated initial-root parent memory buffer and
memfd. An unrestricted inspector successfully reads the buffer, writes the
same fixed canary bytes back, duplicates the descriptor, queries signal
permission with kill(pid,0), and opens user/mount/PID/network namespace handles.
The inspector in a user namespace mapped0..65535 to100000..165535 cannot perform
those operations. Memory/descriptor/signal checks return EPERM; namespace-handle
opens return EACCES/EPERM. The PID and proc status remain deliberately visible,
and pidfd_open succeeds, so denial is not attributed to hiding or stale PIDs.
No command policy is installed for this separate boundary probe. The parent
checks its canary memory and file bytes after both inspectors. No real service,
filesystem-worker data, secret, host process, or namespace is accessed.

These are distinct mechanisms: valid self-access establishes policy dispatch;
the cross-process control establishes the mapped-identity boundary against an
initial-root target on this guest. It does not qualify a same-UID worker,
every ptrace request, ancestor handles supplied through another channel, or
the production executor. No tracing request targets the parent; TRACEME is
limited to the disposable child. Subprocess timeouts bound the controls.

The first positive control failed with EINVAL due to a variadic ctypes scalar
width error. The corrected wrapper passes full-width scalar syscall arguments.
That failed fixture is retained in ../reproduced/process-valid-arguments-negative;
it is not a sandbox finding. An inadvertent second run of the unchanged fixture
also failed before the correction and is retained locally as
/tmp/conglobate-policy-process-valid-v2. No negative result counts as a pass.

Primary references checked2026-10-20:
https://man7.org/linux/man-pages/man2/process_vm_readv.2.html
https://man7.org/linux/man-pages/man2/pidfd_getfd.2.html
https://man7.org/linux/man-pages/man7/user_namespaces.7.html
The process-memory and descriptor APIs use ptrace access permission checks.
Capabilities in a newly created user namespace do not grant capabilities in
its parent. The observed EPERM results are consistent with these mechanisms;
this probe does not independently isolate every LSM/credential check involved.

Worker identity and PID namespace matrix
----------------------------------------
The ordinary policy fixture now runs worker-identity.py. Retained evidence:
../reproduced/worker-identity. A disposable worker-shaped process owns only a
fixed memory canary and memfd, in a user namespace mapping0..65535 to outer
100000..165535. It retains namespace-root capabilities. Inspectors initially
share its PID namespace and can see its PID. No command filter is installed
for this matrix. Results:

Inspector                         Memory read / getfd     Signal-zero queries
Initial-root controller           allowed / allowed       allowed
Sibling userns, same outer UID     EPERM / EPERM            allowed
Same userns as worker             allowed / allowed       allowed
Sibling userns, disjoint UID map   EPERM / EPERM            EPERM

Signal-zero queries use kill(pid,0) and pidfd_send_signal(pidfd,0). They check
permission without delivering any signal. Memory access only reads a dedicated
canary; getfd duplicates only its disposable memfd. The worker checks both
canaries unchanged at shutdown. The same-userns inspector is entered by the
trusted controller using nsenter, and namespace identity is asserted. This is
a control arrangement, not an image command acquiring a namespace handle.

Two further controls deliberately pass a valid worker pidfd into a fresh child
PID namespace, with either the sibling same-UID userns or shared worker userns.
Both pidfd_send_signal(...,0) calls fail with EINVAL. fstat confirms the supplied
FD exists. This exercises the PID-namespace relationship check, not descriptor
closure. Production command entry must still close inherited handles. The
existing restricted self-access tests and full entry/FD tests run afterward.

Worker lifetime has a25-second alarm, outer driver20-second timeout and bounded
inspector subprocesses. Parent cleanup kills/reaps only its owned test worker
if a check fails. No host process, service, actual filesystem worker, secret or
external network is used. This is not a sandbox breakout demonstration: PID
isolation and command policy are intentionally absent from the first matrix.
It does show that same UID/userns alone does not isolate the trusted worker.
Disjoint identity denies these visible-PID operations in this fixture, but
filesystem metadata semantics and the final combined arrangement need separate
qualification. Reused identities, same-userns capability-reduced configurations,
other process interfaces and full integration remain outside this matrix.

Primary references checked2026-10-20:
https://man7.org/linux/man-pages/man2/kill.2.html
https://man7.org/linux/man-pages/man2/pidfd_send_signal.2.html
Signal permission can follow matching real/effective and target real/saved UIDs;
namespace CAP_KILL is an alternative. pidfd signaling also checks the caller's
PID namespace relationship to the target. These explain the observed contrast;
we do not infer that every signal type or process API has identical rules.

Mount API coverage follow-up (2026-10-21)
----------------------------------------
The source review found open_tree_attr and fspick missing from the denylist.
A benign owned-directory detached-mount control reproduced open_tree_attr
acquisition in both restricted steps. The current helper explicitly denies both
APIs. Valid-object and null-argument checks require EPERM; an owned unrestricted
fspick control establishes syscall availability. See MOUNT-API-REVIEW.txt for
source/ABI details and retained negative/fixed evidence. This does not make the
default-Allow policy a complete interface allowlist or qualify newer query APIs.
