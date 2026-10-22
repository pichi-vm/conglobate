# Integrated command execution evidence

This fixture combines production private mount/idmapped attachment, registered
cgroup containment, helper observation, cancellation and child reaping with an
actual shell running in an ext4 image. It is not the production executor or full
sandbox qualification. All mounts, namespace and capability operations occur
inside a disposable QEMU guest; the host only builds and packages the fixture.
The guest has no host disk/share or network device.

## Reproduction

Requires the preserved seed at
`/var/tmp/conglobate-cap-review-fix-normal-v1`, Fedora kernel modules for
`7.2.10-200.fc44.x86_64`, QEMU and the tools named in run.py. Build with:

```
TMPDIR=/var/tmp/conglobate-rewrite-tmp cargo build --locked --manifest-path validation/rewrite-command-integration/Cargo.toml
python3 validation/rewrite-command-integration/run.py /var/tmp/NEW-UNUSED-DIRECTORY
```

The seed supplies the guest initramfs tools and QEMU arguments. Per-run input
hashes, serial logs, command, source snapshots and verdicts are in evidence/.
Large guest artifacts remain at the paths in provenance.json; these temporary
paths must be durably archived before reference retirement. This harness has its
own pinned Cargo.lock. Sources use safe Rust and methods on the fixture object.

## Results

- v1: initial successful command write and timeout/cgroup/reaping checks.
- v2: stronger privilege assertions failed before command startup; no full pass.
- v3: traced failure: Bubblewrap --cap-drop ALL left a nonzero bounding set.
- v4: explicit setpriv finalization cleared the bounding set before the shell.
  Both command cases, filesystem check and remount passed; fixture cleanup failed
  because its initramfs lacked rmdir. The overall run correctly failed.
- v5: added rmdir; all assertions and fixture cleanup passed. Clippy with warnings
  denied and formatting also pass for the fixture.

v5 checks all five capability sets are zero, no_new_privs is one, UID mapping is
0 -> 100000 for 65536 IDs, selected supervisor paths are absent, and shell writes
appear as UID 0 in the original filesystem. It checks normal completion and a
200ms timeout, domain removal, helper reaping, no remaining adopted children,
unchanged supervisor sentinel, clean unmount, e2fsck and read-only remount data.
These are benign assertions over owned fixture resources, not escape attempts.

## Limits and implications for production

Bubblewrap, nsenter and setpriv are temporary test adapters. The fixture adds
SETPCAP solely to its trusted finalizer and drops the bounding set before exec;
the command shell has no capabilities. This is not the final OS-build allowlist.
The finalizer is copied into this fixture's freshly constructed image, whose
contents are known. Production must not execute a trusted finalizer from an
untrusted image path or rely on its loader/libraries; it needs trusted executable
and dependency authority until command admission.

No scoped labeling authorization, final seccomp policy, metadata/SELinux package
transaction, network-enabled step, dual-mount bootstrap or direct scute sealing
is integrated here. Tests do not establish complete FD/path confinement. The
fixture discovers a generated mount path; production requires a scoped handle.
The timeout kills the helper and leaves an empty mount-point directory, which
fixture cleanup removes only after unmount. Production ordinary-error ownership
must handle this without a serialized next-invocation recovery requirement.
RecordedDomain is reused as an existing containment adapter, not an endorsement
of the superseded guardian architecture. The v6 supervisor-death case below demonstrates VM disposal with live commands.
Failed startup aborts
the guest fixture, so partial-startup cleanup still needs integrated coverage.


## Dedicated-VM supervisor death

Run with a new output directory and `--supervisor-death` as the final argument.
v6 starts a continuously running contained command, then its independent guest
controller terminates the supervisor with SIGKILL. It verifies wait status 137
and a still-populated command cgroup, then returns to guest PID 1, which powers
off the VM through sysrq. QEMU exits; the serial log includes kernel Power down.
There is no guardian or next invocation, and no command-success marker appears.
The controller and PID 1 are outside the supervisor's process/cgroup lifetime.

This demonstrates the fixture environment contract, not the eventual appliance's
integration. No inventory/publication implementation is exercised; absence of a
success marker is not artifact rejection coverage. Active-policy lifetime during
the shutdown interval remains outstanding. The helper may survive until VM
shutdown by design. v7 reruns normal completion, timeout and filesystem checks
with the same updated sources and passes. Both runs and their init scripts are
preserved; Clippy and formatting pass.


## Native privilege finalization

The production CommandPrivileges policy now applies through safe rustix thread
APIs. It is immutable and consumed by application. Kernel probing covers all 64
v3 capability bits; unsupported bits are ignored, supported disallowed bits are
dropped from the bounding set before effective SETPCAP is removed. Ambient and
inheritable sets are empty and no_new_privs is set. Missing required bounding
capabilities fail; any partial mutation error requires helper exit.

The candidate filesystem profile retains CHOWN, DAC_OVERRIDE, FOWNER, FSETID,
KILL, SETGID, SETUID, SYS_CHROOT, SETFCAP and NET_BIND_SERVICE. It excludes
SYS_ADMIN and SETPCAP. This is not a qualified distro compatibility contract.

v8 verifies exact effective/permitted/bounding sets and empty inheritable/ambient
sets after shell exec in the trusted guest. v9's positive check also passes inside
a user namespace, but the negative test was incorrectly ordered: dropping CHOWN
before unshare did not remove it in the new namespace. That overall failed run is
preserved. v10 drops CHOWN inside the new namespace and verifies rejection before
shell execution. It passes this negative, the positive user-namespace exec check,
and the normal image command/timeout/remount regressions. Production-source
snapshots are included in these runs' evidence.

This native component is tested in a separate trusted guest process. The image
command fixture still uses its earlier temporary launcher/finalizer adapter.
Moving native finalization into the complete trusted launch path, syscall
restrictions and active LSM admission remains required. Creating a new user
namespace after capability finalization must be denied by that syscall policy.


## Native syscall restrictions

CommandSyscalls compiles an immutable native-ABI default-deny filter using
[seccompiler 0.5.0](https://docs.rs/seccompiler/0.5.0/seccompiler/), licensed
Apache-2.0 OR BSD-3-Clause. Application Rust remains safe; linux-raw-sys provides
architecture-specific constants, not application syscall FFI. The profile starts
from the preserved capability-review ordinary syscall list, masks namespace
flags in clone, and returns ENOSYS for clone3 via a separately installed filter.
Both compilation and installation errors abort command setup. Mount, namespace,
keyring, BPF, io_uring and process-inspection syscalls are absent from the allowed
list. Socket families, prctl and ioctl have argument rules. This is a candidate
profile: package-manager and full amd64/arm64 ABI qualification remains required.

v11 failed in Rust Command::output: std read_output uses FIONBIO to make pipes
nonblocking, and the prototype ioctl list omitted it. The local Rust standard
library paths sys/process/mod.rs, sys/process/unix/common.rs and sys/fd/unix.rs
identified this operation. v12 permits that descriptor-local operation, retaining
all other ioctl restrictions. It passes native capability finalization followed
by filter installation, thread creation/join, owned-file write/read/delete,
subprocess capture and denial of unshare --user, then execs a shell that checks
its exact capability sets and no_new_privs. Existing image execution, timeout and
filesystem persistence checks also pass. The failed run and source snapshots
remain preserved. Root cargo test output is retained with v12.

The filtered process is a separate trusted fixture process; the actual image
command path still uses the earlier launcher. Native syscall and capability
finalization must next be integrated with trusted root setup and active labeling
policy. Denial of every individual excluded syscall, compat-ABI behavior, ioctl
argument cases and real package transactions are not demonstrated by this run.
No arm64 runtime or confidential-computing qualification is claimed.


## Native root transition and image execution

v13-v18 replace the Bubblewrap/image-setpriv command path with native CommandRoot
and EnteredRoot. Trusted code loads from the guest before pivoting, so an image's
loader or libraries cannot run with setup privileges. The object borrows its
PrivateMountNamespace; selected roots must be opened after entering that scope.
It clones/attaches the assigned mount tree through descriptors, pivots, detaches
the old root, and drops root descriptors. EnteredRoot verifies real procfs and
close-on-exec flags for nonstandard descriptors, applies native privileges and
syscalls, then execs the image shell with explicit environment and null stdin.
Any transition/exec error requires helper exit. No rollback claim is made.

v13-v16 failed during root setup and remain preserved. An fd opened before
unsharing referenced the old mount tree; after fixing that order, pathname self
binding still left cwd on the covered mount rather than the new view. v17 uses
open_tree/move_mount plus fchdir on the new mount descriptor and passes. v18 also
removes setpriv from the image, asserts its absence, and passes the same normal
completion, timeout, capability, write/ownership and fsck/remount checks. The
shell now has the candidate filesystem capability profile, not the earlier
all-zero profile. Root tests (including fourteen compile-fail doctests), root
and fixture Clippy, and formatting pass. Source-grounded comparison consulted
Bubblewrap's pivot sequence; no implementation code was copied from it.

This remains an integration fixture, not the complete launcher. External nsenter
and unshare establish user/PID/network/IPC/UTS namespaces; trusted mount prepares
procfs on the known fixture image. Production needs confined runtime mount
construction, device/proc/sys restrictions, namespace init behavior and descriptor
negative tests. The root selector still uses a fixture-discovered path. Empty
mount-point cleanup on timeout is fixture-owned. Active LSM admission, CARAPACE
environment, complete error/timeout ownership and distro qualification remain
outstanding. The v6 VM-disposal test has not yet been repeated on this native root
path. Earlier sections describe their historical versions, not the current path.


## Native confined runtime mounts

v19 replaces the external mount executable in the command setup path. Native
RuntimeTargets resolves existing proc/sys directories with openat2 relative to
the pinned image root, rejects magic links/mount crossings, and treats absolute
symlinks as image-relative. Both targets are resolved before attaching either
mount. Root aliases, duplicate targets and nested targets are rejected, avoiding
one runtime mount hiding another. Safe fsopen/fsconfig/fsmount/move_mount APIs
attach fresh procfs and an empty read-only tmpfs at sys. Both use nosuid/nodev/
noexec. Detached resources close on failure; attached mounts belong to the private
command namespace, and setup failure requires helper exit. No scaffolding is
created in the image by this implementation; absent directories currently fail.

Four maintained tests cover confined absolute symlinks, outside-sibling rejection,
root/duplicate aliases and nested targets. v19 passes actual native image command
setup and explicitly verifies /sys rejects writes, plus existing capability,
namespace-denial, timeout, ownership and filesystem persistence checks. Root
cargo test, root/fixture Clippy and formatting pass.

v20 exercised independent VM shutdown on this native path: supervisor wait status
137, a populated command domain and kernel power-down are present. Its overall
verdict failed because printk interrupted the status marker. The parser now
removes timestamped kernel records only when reading userspace markers, while
still requiring the raw kernel power-down message. A directed parser check
accepts the interrupted status 0 and rejects interrupted status 1. v21 is a fresh
passing run. v20 remains preserved with its original failing verdict.

This closes the former external proc-mount adapter and native-path VM-disposal
coverage gaps. It does not qualify active LSM lifetime, private device/devpts/shm
setup, sensitive proc masking, missing-directory scaffolding, namespace init,
complete cleanup ownership, or appliance/architecture/distro compatibility.


## Read-only proc control views

v22 adds ProcRestrictions to native runtime setup. After mounting fresh procfs,
it resolves kernel-generated sys, sysrq-trigger, irq and bus entries through the
proc mount descriptor, clones each view and attaches it read-only with nosuid,
nodev and noexec. Symlinks and mount crossings are rejected. Kernel-configuration
ENOENT is tolerated because no such interface exists; all other errors abort.
Partial attached setup remains owned by the private command namespace, whose
process must exit on failure. No command is admitted during construction.

The actual image shell counts four expected read-only mountinfo entries on this
known guest kernel and verifies that opening sysrq-trigger for output fails.
It performs no write to a kernel control. v22 passes these checks plus native
execution, capability profile, timeout, reaping and filesystem persistence.
Repository tests, root/fixture Clippy and formatting pass. Logs and source hashes
are retained. Other sensitive proc information masking and private device/runtime
construction still require work; this is not a complete sandbox qualification.


## Private runtime device inodes

v23-v28 add CommandDevices: the trusted initial-user-namespace helper creates a
fresh detached tmpfs per command, with mapped root ownership and private null,
zero, full, random and urandom character devices. Permissions are set explicitly
independent of umask. fd/stdin/stdout/stderr are symlinks to command procfs. The
mount is nosuid/noexec, deliberately not nodev; image mounts remain nodev. No
supervisor device inode, block device, fuse or control device is bound into it.
The device mount target participates in root/alias/overlap confinement checks.

v23-v25 failed closed at the descriptor audit. Diagnostics found an extra null
FD: Rust's startup sanitize_standard_fds polls standard streams and treats the
O_PATH fsmount descriptor as invalid, opening an inheritable null descriptor.
The local std sys/pal/unix/mod.rs source explains that behavior. v26's readable
reopen avoided that issue but lost the original detached-mount ownership and
failed attachment. All failures and diagnostics are preserved.

The final handoff queues the original mount FD over a private Unix datagram
socket using SCM_RIGHTS. This keeps its file ownership alive until receipt and
passes a pollable stdin through the trusted namespace adapter. The receiver uses
CMSG_CLOEXEC/DONTWAIT and requires the exact three-byte CBOR packet [1,0], exactly
one descriptor and no truncation. A tmpfs check is not sender authentication: the
channel is restricted to trusted setup, and the final shell receives null stdin.
No descriptor-audit exemption was added. Packet failure cases still need dedicated
coverage before full launcher acceptance.

v27 passes the corrected handoff. v28 additionally verifies exactly nine /dev
entries, mapped-root device ownership, working null output, expected full-device
write failure, and absence of runtime null/fd entries after image unmount/remount.
Command, timeout, proc restrictions, filesystem checks and reaping regressions
pass. Root tests, root/fixture Clippy and formatting pass. Root tests are preserved
with v28. Terminal/session isolation, tty/devpts/shm, remaining proc information
masking, active LSM and complete cleanup/qualification remain outstanding.


## Private devpts, shared memory and session

v29 creates a new session before exposing a fresh private tty inode. Device setup
adds trusted pts/shm directories and a relative ptmx symlink. After attachment
inside the command user namespace, native filesystem-context APIs mount a new
private devpts instance (ptmxmode 0666, slave mode 0620, namespace gid 0) and a
separate mode-1777 tmpfs at shm. Both are nosuid/noexec; shm is additionally nodev.
No VM devpts or shm mount is shared. Missing or failed setup rejects execution.

The actual image shell checks thirteen expected /dev entries, denied access to
an inherited controlling tty, PTY master allocation with a new pts/0 slave,
slave removal on master close, and writable fresh shared memory in both normal
and timeout steps. Read-only image remount confirms pts/shm were not persisted.
Existing capability, proc restrictions, timeout, reaping and fsck checks pass.
Root tests, root/fixture Clippy and formatting pass; evidence includes source
snapshots and test output.

This does not test unlocking the slave, terminal configuration, interactive jobs,
or full terminal ioctl compatibility. The filter still needs those qualification
cases. It also does not replace the fixture's external namespace adapters or add
namespace init behavior. Active labeling-policy admission, complete partial-setup
ownership, control-channel negative tests and distro/platform qualification remain.
