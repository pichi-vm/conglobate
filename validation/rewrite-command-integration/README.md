# Integrated command execution evidence

This fixture combines production private mount/idmapped attachment, the typed
policy/admission/teardown step, cgroup containment and child reaping with an
actual shell running in an ext4 image. It is not the production executor or full
sandbox qualification. All mounts, namespace and capability operations occur
inside a disposable QEMU guest; the host only builds and packages the fixture.
The guest has no host disk/share or network device.

## Reproduction

Requires the preserved seed at
`/var/tmp/conglobate-cap-review-fix-normal-v1`, Fedora kernel modules for
`7.2.10-200.fc44.x86_64`, QEMU and the tools named in run.py. Run with:

```
TMPDIR=/var/tmp/conglobate-rewrite-tmp python3 validation/rewrite-command-integration/run.py /var/tmp/NEW-UNUSED-DIRECTORY
```

The runner requires a successful build before packaging the binary and retains
compiler output in build.txt. The seed supplies the guest initramfs tools and
QEMU arguments. Per-run input
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


## Active labeling and supervisor loss

v30 rejected the fixture's default mode-777 bpffs root before loading commands;
this failed run is preserved. The fixture now protects that mount root with mode
700. v31 passes policy-disabled denial, enabled set/read/remove of security.selinux
on an assigned ext4 file, rejection on the private /dev/shm filesystem, and denial
of security.selinux.extra. Commands use the existing restricted native execution
path and no CAP_SYS_ADMIN. Normal completion and timeout both cancel the domain,
reap the helper and drain descendants before disabling/removing the policy.

v32 kills and reaps the supervisor with a command still running. Only after death,
the independent controller signals the test command through a fixture-owned image
file. The command relabels the assigned file, reads back its new label, confirms
labeling private shared memory is still denied, and writes an acknowledgement.
The controller then powers off the VM; no command-success receipt is accepted.
This proves actual label/guard behavior during this fixture's death interval,
not merely survival of pin names. The VM's original ext4 mount, user-namespace
holder and bpffs remain live through poweroff. There is no guardian.

EnabledPolicy is a validation-only adapter: activation uses a pinned map handle,
and ManuallyDrop prevents an error from running the disabled owner's destructor
against active guards. Explicit teardown is invoked only after the fixture's
complete cancellation/reaping/drain sequence. Production still needs an aggregate
owner that enforces that sequence, retains identities, gates command admission,
and rejects output after failed cleanup. The adapter is not that API. External
namespace setup, fixture control-file synchronization, incomplete failure coverage
and lack of architecture/distro qualification remain explicit limitations.


v33 exposed a real completion race while adding label readback after remount:
channel closure was treated as process exit, so cgroup cancellation could kill a
successful helper during its exit sequence. Production now offers a bounded
pidfd exit observation distinct from channel loss. The fixture observes actual
exit before cancelling residual descendants, then still reaps and drains them.
No sleep was added to production. v34 passed with a timing-delay regression
fixture and also confirmed label persistence after unmount, e2fsck and read-only
remount. Its timing-based fixture is superseded by an explicit handshake: the
helper binds a private Unix datagram socket before closing its channel, then
blocks for release. The supervisor proves exit observation still times out,
sends the release, and awaits actual exit. This forces the ordering without
assuming a scheduling window. Both sides bound the fixture socket wait.

v35 passes the explicit-handshake regression, active label checks, timeout,
filesystem verification and label persistence after remount. Root tests and
root/fixture Clippy/formatting pass. Full sandbox qualification remains false.


## Resident command namespace init

v36 adds production NamespaceInit after root entry, capability finalization and
seccomp. The image shell becomes its child instead of PID 1. Init blocks in
waitpid(__WALL), reaps adopted children while the shell runs, and exits with the
shell outcome (signal termination maps to 128 + signal). Kernel PID-namespace
teardown stops remaining members; outer cgroup cancellation/reaping/draining
still precedes policy removal. The helper remains exit-only on setup/wait errors.
No scheduling sleep or signal-handler loop implements this behavior.

CommandRoot now consumes PrivateMountNamespace. Successful pivot/detach closes
its restoration descriptor before returning EnteredRoot. The resident init must
retain no extra descriptor even if CLOEXEC: its pre-spawn audit now rejects all
nonstandard descriptors except owned null input, which is moved to stdin and
closed. Init is non-dumpable before spawning, preventing same-UID commands from
opening its executable, memory, maps, environment or fd/0 through /proc/1. The
image child's exec restores ordinary child dumpability. Commands still receive
the documented environment and no supervisor filesystem/root capability.

The fixture asserts shell PID != 1 and PPID == 1, probes those denied proc views,
and uses explicit file release to let an orphan exit. Its disappearance from proc
while the shell remains alive demonstrates init reaping. v36 passes the existing
policy, capability, runtime, timeout and label-persistence checks.

v37 exposed the fixture's stale empty target directory after timeout when adding
a fourth step. Selection now checks the actual mount's device/inode identity;
production still needs scoped root transfer rather than path discovery. The
concurrent root-test run also exposed an immediate-EOF assumption in a descriptor
closure test. It now bounds its EOF read rather than assuming no transient
CLOEXEC reference can exist during another test's process spawn. Both failures
are preserved; production descriptor-transfer behavior was unchanged.

v38 passes shell exit-code 37 propagation, cancellation after that failure, and
successful shell exit with a deliberately live background child. The child
acknowledges startup before shell exit. Completion and an empty cgroup establish
that it cannot survive the step. Preserved kernel source excerpts explain
PID-namespace teardown's signal/reap ordering; they do not replace the tests.
v39 repeats supervisor SIGKILL with active labels using the resident init, checks
continued image labeling and denied shared-memory labeling, then observes actual
VM poweroff. Both runs pass. Root tests, Clippy and formatting pass.

Remaining work includes native namespace creation (external nsenter/unshare still
construct them), production policy admission/ownership and complete failure
cleanup, runtime scaffolding, further proc masking, PTY compatibility, explicit
diagnostic plumbing, distro and architecture coverage. These fixtures do not
claim to qualify the complete sandbox or all signal/clone combinations.


## Native command namespace construction

v40 replaces command-path nsenter/unshare with production CommandNamespaces in
an isolated trusted setup process. Safe crate APIs clear supplementary groups,
join the selected user namespace, set UID/GID 0, unshare mount/PID/network/IPC/UTS,
and make mount propagation recursively private. The thread-affine owner consumes
itself when spawning the trusted root-entry child (PID 1). The helper retains no
namespace FD and cannot restore supervisor authority. Returned process ownership
uses the existing termination/reaping type. Errors require the helper to exit.

v41 verifies changed mount/net/IPC/UTS and pid-for-children namespace identities,
unchanged current PID namespace in the intermediate helper, the exact selected
user namespace, all four UID/GID fields equal zero, no supplementary groups, and
no shared/slave mount propagation. A separate fixture process rejects a regular
file and a synchronized multithreaded setup attempt before transitions. No delay
is used to establish the extra thread's existence. Existing image commands still
prove PID 1 init behavior, restricted capabilities, labels and cleanup.

v42 removes nsenter/bwrap from the guest and deletes unshare after the initial
user-namespace holder and preflight probes are established. Every command then
passes using native construction, including shell failure and timeout. v43 checks
supervisor death and actual VM poweroff through that same path, with live positive
and negative label checks after supervisor loss. Both pass. The fixture's 30-second
intermediate wait is a test bound, not a recipe timeout default.

The initial mapped user namespace is still created by an external fixture holder;
production allocation/mapping/pinning must replace it. Scoped root transfer,
full step admission and policy ownership, network provisioning, failure cleanup
and broader distro/architecture qualification remain unfinished. This change
removes command-path runtime-tool dependence; it does not complete M4.


## Native mapped user namespace and creator lifetime

v44 replaces the sleeping external user-namespace holder. Every step starts a
short-lived trusted creator using production MappedUserNamespace::serve. It
unshares NEWUSER and exchanges identities only afterward. The supervisor writes
validated nonzero UID/GID ranges to the empty mapping files, retains the namespace
FD, closes the private channel to release the creator, and reaps it. Only then
can create return its namespace owner. Consuming pin uses the existing protected
tmpfs namespace-pin owner; no serialized namespace record or guardian is needed.
The fixture removes the pin only after descendant drain and policy removal.

v45 additionally resolves the creator's proc directory through its owned pidfd's
Pid field, which the kernel reports relative to the procfs instance. It requires
a match with the exclusively wait-owned child before accessing maps, checks a
new namespace identity and empty maps, and writes each map once at offset zero.
A creator deliberately omitting NEWUSER is rejected before touching the parent's
mapping. Its retained child owner is reaped, and the unchanged parent map and
empty child list are verified. The image checks that setgroups remains allowed.
Range boundary tests reject zero/empty/overflow/invalid-ID mappings.

v46 terminates the supervisor after the creator has already been reaped. The
protected namespace bind mount survives; label positive/negative checks pass in
the surviving command before independent VM poweroff. Both runs pass without
runtime tools available during namespace or command construction. All mounts,
mapping writes and capability operations occur only inside the disposable VM.
Root tests, Clippy and formatting pass; evidence and kernel-source excerpts are
preserved. An initial ownership compile error is documented in BUILD-NOTES.txt.

The fixture still adapts pin paths and mounted-root discovery to process startup,
and policy activation remains fixture-only. Production step admission, scoped
root/namespace handoff and complete failure ownership remain unfinished. Late
partial-map failures and deployment coverage are not qualified by these tests.
The mapped range is supplied by trusted construction; this does not implement a
general concurrent shared-host UID allocator or claim arbitrary OS ID coverage.

## Owned mount handoff

v47 replaces mounted-root directory discovery with Mount<Attached>::launch. The
mount owner sets the trusted child's cwd before exec; the child inherits cwd
through namespace cloning, then CommandRoot::from_current_directory captures
that cloned view. The kernel namespace source excerpt retained with v50 shows
cwd retargeting. No generated mount path is passed to root entry.

The consuming launch returns Mount<InUse>, retaining both mount and process.
Failed spawn returns the original Attached owner. A wait deadline returns InUse
so the caller can retry or terminate the direct child. Reaping returns Attached
and the exit status; neither proves descendant quiescence or finalization.
A compile-fail check prevents explicitly closing InUse.

v48 adds an unrelated mount-name decoy and exercises failed spawn, an explicitly
IPC-blocked child, immediate deadline failure and successful retry. v49 also
terminates a blocked child and verifies its signal status and retained mount
identity. v50 exercises supervisor death and active scoped labeling until actual
VM poweroff. All pass. Root tests, root/fixture Clippy and formatting pass.

These tests do not complete step admission or policy ownership. Namespace pin
paths remain a fixture adapter, as does active-policy construction. The earlier
mounted-root discovery limitation is resolved; complete partial-setup cleanup,
production lifecycle aggregation and broader qualification remain outstanding.

## Event-driven direct-child waits

v51 passes the existing normal, failed-command, timeout, retained-root ownership
and direct-child termination checks after replacing Process wait/destructor sleep
loops with pidfd observation and separate status collection. Identified children
reuse the transferred pidfd. Other directly owned children open a pidfd while
exclusive wait ownership retains their PID, including after exit but before reap.
This requires normal SIGCHLD zombie semantics and no competing reaper.

Host process tests explicitly block on pipe input, test an expired deadline,
then release by EOF or terminate. No delay establishes test ordering. The root
suite, Clippy and formatting pass. Cgroup-empty and adopted-child-drain polling
remain separate outstanding event-wait improvements; this is not full M4 or
production active-policy qualification.

## Cgroup change notifications

v52 passes cancellation/removal through a persistent cgroup.events watcher rather
than a sleep loop. It reads populated, polls PRI while populated, then rewinds
and rereads. Kernfs's event counter covers the read-to-poll interval; its normal
PRI|ERR notification is not treated as a failure. The retained kernel excerpts
show read-counter updates, poll registration/checking and recursive population
notification semantics. Only populated=0 allows directory removal, and separate
reaping/drain gates remain necessary.

The existing normal/timeout/failure integration cases and root tests pass, as do
Clippy and formatting. This run does not instrument every notification interleaving
or deterministically force a change inside the read/poll interval. Adopted-child
drain still polls and production policy/lifecycle integration remains unfinished.

## Adopted-child drain without sleeps

v53 adds a dedicated single-threaded reaper probe. A child is blocked on pipe
input, its individual Child owner is relinquished, and drain with an immediate
deadline must return the reaper owner. EOF releases the child; retry collects one
status and observes ECHILD. This and the normal/timeout/failure pipeline pass.
Root tests, root/fixture Clippy and formatting pass.

Production drain now selects a child from procfs, opens/checks its pidfd and
waits for exit before resuming waitpid. Normal SIGCHLD and exclusive reaping
retain the selected PID until collection. The procfs list is not an atomic
snapshot or a completion receipt: inconsistencies fail, and only ECHILD ends
drain. No process-wide signal masks change. The retained kernel excerpt includes
the iterator's limitations. This does not complete production step ownership.

## Typed pinned namespace identity

v54/v55 use UserNamespace<Mapped> -> UserNamespace<PinnedNamespace>. Pinning now
retains the mapping description along with namespace lifetime ownership. Policy
scope is derived from that owner, replacing the fixture's pathname-based inode
lookup and repeated hard-coded root UID. Failed release returns either the mapped
pin owner or an unmounted target needing unlink. Compile-fail tests reject scope
construction before pinning and pinning an already pinned owner.

v54 passes normal/timeout/failure execution; v55 passes supervisor death and
scoped labeling until VM poweroff. Root tests (18 compile-fail), root/fixture
Clippy and formatting pass. This removes policy identity reconstruction only:
namespace paths still adapt trusted helper startup, assigned-filesystem lifetime
is fixture-owned, and production policy activation/admission remains unfinished.

## Filesystem identity pins

v56 adds production FilesystemPin ownership: a non-recursive bind mount in the
protected initial mount namespace. Its descriptor-free public interface exposes
checked metadata and explicit release only. Dropping a live pin leaves the
kernel mount intact; empty-target cleanup uses rmdir and cannot recursively
traverse image contents. A regular file is rejected before target creation.

The dedicated probe unmounts the original ext4 source while retaining the pin,
checks device/inode identity, restores the fixture source and releases the pin.
The command pipeline derives policy device identity from this owner and releases
it only after command drain and policy removal. v56 passes all normal/timeout/
failure cases. An initial fixture-only rustix dependency compile error and its
correction to existing fixture setup tools are recorded in BUILD-NOTES.txt.

v57 kills the supervisor and verifies its pin is still mounted. The independent
controller unmounts the original source and uses the retained pin to coordinate
and observe surviving-command label checks before VM poweroff. It passes.
Root tests, root/fixture Clippy and formatting pass. This proves the pin primitive's
lifetime behavior, not complete production admission/policy ownership, filesystem
owner-namespace derivation, storage construction, or image finalization.

## Bound disabled policy ownership

v58–v61 bind the disabled policy to its namespace and filesystem pin owners.
Binding checks the actual namespace/root mapping and exact filesystem-device
membership. Missing or mismatched pins fail while returning all inputs; the
fixture retries with the correct pin. Owner-namespace provenance remains a
trusted filesystem-construction prerequisite, not something stat can prove.

The cleanup test creates a benign directory under the private bpffs pin directory
only after command drain and config disable. Policy close removes its known pins
but cannot remove the nonempty directory. Identity pins remain held. Removing
the blocker permits cleanup retry, after which the identity owners are returned
for explicit release. v58 and v59 pass; v60 verifies supervisor-death persistence.

Review rejected the original cleanup error's ready-policy type. The final API
returns a cleanup-only BoundCleanup owner, preventing future reactivation after
partial teardown. v61 passes this corrected path. Pre-correction snapshots and
review notes are retained. Root tests (20 compile-fail), root/fixture Clippy and
formatting pass. Production activation and command admission are still unavailable;
this is ownership integration, not completed M4 qualification.

## Explicit command admission

v63/v64 add a Registered -> Admitted handshake after cgroup registration. A helper
blocks before image execution until it receives admission, then acknowledges it.
Packets carry no descriptors and unexpected phases fail. ContainedHelper preserves
its protocol state; a failed admission returns a cancellation-only owner. Missing
ACK is not proof the command never started, so all error paths require cancellation.

The new admission-timeout mode expires the parent deadline before send, cancels
and drains the waiting helper, removes policy and verifies no command marker or
children. Normal/timeout/failure cases pass in v63; supervisor-death pin/label
persistence and VM poweroff pass in v64. Root tests (21 compile-fail), root/fixture
Clippy and formatting pass. This protocol does not itself certify sandbox readiness
or policy activation: the enclosing production step still needs to enforce both.

v62 is preserved as INVALID evidence: after an E0515 fixture build failure, the
agent mistakenly ran the new script with the preceding binary. Its failure and
provenance are retained. The runner now builds with check=True, retains build.txt,
and aborts before guest construction on build failure. Fixture error conversion
was corrected without weakening production resource-owning errors.

## Production policy and command step

v65–v67 use production Step for all enabled-policy command cases. The step consumes
bound policy, a registered helper and the installed reaper. Activation precedes
admission; active configuration has no public mutation/removal interface. Its
sealed states require cancellation, helper reaping, descendant drain and domain
record cleanup before disabling policy and returning its identity-owning payload.
Failures preserve the applicable phase; unfinished Drop retains policy/identity
pins until VM disposal. There is no separate guardian process.

v65 passes normal/shell-failure/execution-timeout/admission-timeout behavior and
partial disabled cleanup retry. It also rejects creating policy pins in a private
helper mount namespace. v66 passes supervisor death, active label checks and
filesystem/namespace pin persistence until VM poweroff. v67 additionally cancels
a Ready step before admission and confirms no command marker or children remain.
Its disabled baseline no longer has any map-update code; the fixture's direct
libbpf-rs dependency was removed. Root tests (24 compile-fail), root/fixture Clippy
and formatting pass. Earlier compile diagnostics and snapshots remain preserved.

The Step constructor still requires trusted helper configuration to match its
bound scope. It is not a complete recipe runner: scoped namespace transfer,
filesystem-owner provenance, storage/workspace assembly, complete setup/teardown
fault coverage and broader deployment qualification remain unfinished. The
recorded-domain adapter is transitional, not a new cross-invocation service.

## Attached control-mount verification

v68 rejects a control directory whose descriptor points into a foreign/detached
mount even though the caller has returned to init's mount namespace. It creates
private bpffs and tmpfs mounts, retains their descriptors, restores init's scope,
and checks that policy-pin and recovery-directory construction both fail for
those descriptor paths. Valid control mounts still pass the full normal step
suite, including admission failure, pre-admission cancellation and cleanup retry.

Production verification compares descriptor STATX_MNT_ID with the kernel's current
thread mount table, after checking init-namespace identity. Step config acquisition
rechecks policy placement. Retained kernel excerpts ground ID correspondence and
lifetime. Missing IDs and inconsistent views fail closed. This proves attachment
at the check; the environment must still retain mounts until VM disposal and
exclude privileged out-of-protocol mutation. Root tests, Clippy and formatting
pass; this does not complete M4 or add new supervisor-death qualification.

## Namespace/device descriptor handoff

v69 (normal suite) and v70 (supervisor death) exercise CommandSetup: the trusted
helper opens its selected namespace once, clones that handle for idmapping, then
transfers it with the original private device mount over a one-shot descriptor
channel. The namespace-entry helper no longer accepts a namespace pathname. After
entry it passes only the device mount to root setup. Namespace identity, runtime
mounts, labels, command restrictions, cancellation, cleanup retry and retained
policy/filesystem identity after supervisor death pass in the disposable VM.

Host protocol tests reject malformed/version-mismatched/truncated packets, extra
or missing rights, and invalid object types; socket EOF checks descriptor cleanup.
Valid received rights are CLOEXEC. This is trusted internal setup, not an
untrusted-input authentication protocol: nsfs validation does not establish user
namespace kind (setns NEWUSER checks that on entry), and a tmpfs directory alone
does not establish device-tree provenance. The supervisor-to-first-helper fixture
path remains; full scope-bound construction is still pending. Source snapshot
notes distinguish VM production sources from subsequently added unit tests.

## Event-driven fixture readiness

v71 is a preserved compile failure (no guest boot): replacing the marker polling
loop initially removed a binding still used by final content assertions. v72
normal and v73 supervisor-death runs pass after correction. Fixture readiness now
registers an inotify directory watch before testing marker existence and waits on
kernel events under the setup deadline. Events only prompt another state check;
unrelated events, queue overflow or elapsed time cannot establish success. Peer
loss is checked on each state check; without filesystem events it can be reported
at the bounded deadline. This fixture utility is not a production readiness API.
Host tests cover existing/queued markers, a concurrent writer and unrelated events.
Root tests also replace the last /bin/sleep unit-test hold with a child blocked on
stdin until termination. No sleeps remain in production Rust, current host tests
or this Rust fixture. Historical evidence is preserved unchanged. Shell fixture
coordination and wider lifecycle qualification remain separate work.

## RAM workspace foundation

v74 creates production Workspace inside the disposable VM: a detached tmpfs with
nosuid/nodev/noexec and a private root. Its anonymous scratch files share one
filesystem, have distinct inodes and zero link count, retain sparse zero-fill,
and support write/seek/sync/read. Dropping scratch owners and workspace leaves the
mount table unchanged. The normal command suite also passes. A compile-fail test
prevents dropping the workspace while a scratch lease remains in use. Raw file
or mount handles are not public. Constructor intermediates are RAII descriptors;
no attached pathname needs rollback. This does not establish allocation-failure,
OOM, kernel-device-consumer lifetime or whole-build workspace qualification.
The context, snapshots and device-map owners still require integration. No new
supervisor-death run was needed for this standalone owner; no death claim added.

## Context staging on the workspace

v75 stages a captured Git commit through Workspace::materialize. The staged file
shares the scratch filesystem device and contains the committed bytes. After all
scratch files and the initial workspace owner are dropped, the staged context
still reads correctly and explicitly cleans up through its retained parent
reference. The exact Git input is preserved as context.bundle and its commit ID.
Root tests, Clippy and the normal VM suite pass. This is staging only: the existing
attached-bind read-only transition has not been qualified on a detached workspace.
It must be integrated before this becomes the production build context flow.

## Detached read-only context

v76 replaces the context's attached bind/remount with a cloned detached mount and
mount_setattr read-only enforcement. The workspace probe freezes the staged
context after the initial workspace owner has been dropped, checks EROFS for
metadata mutation, removes backing names while retaining a reader, reads the
committed bytes again and checks that the surviving reader still rejects mutation.
The mount table remains unchanged. An EMFILE setup failure returns the original
staged owner; restoring the descriptor limit permits retry and cleanup. The normal
command suite passes. No attached mount needs busy-unmount recovery in this flow.

The older context-mount harness now reflects these lifecycle semantics; its
historical attached-mount evidence remains preserved. Its stale lockfile initially
prevented a locked check; offline refresh and compilation succeeded. It was not
rerun as a separate VM qualification here. Root tests, Clippy and formatting pass.

## Context cleanup recovery

v77 injects descriptor exhaustion during detached context cleanup after the initial
Workspace owner has been dropped. ContextCleanupFailure preserves the private
backing directory and its parent descriptor; after restoring the limit, retry
succeeds. Retained readers remain read-only. Staged and read-only cleanup share
this owner, and a failed freeze's cleanup uses it too. A cleanup error exposes
only retry, never a partially deleted checkout. The normal VM suite, root tests,
Clippy and formatting pass. This covers EMFILE at removal, not every possible
partial deletion/storage failure or full lifecycle qualification.

## Owned dm-zero mapping

v78 uses production Mapping<Created> -> Mapping<Active> with pinned devmap-linux.
Two simultaneously owned devices have distinct identities. The active dm-zero
returns zeroes through a guest-only block node. Explicit close fails EBUSY with a
live holder and retains cleanup ownership; after closing the holder, retry removes
the device and the node cannot be reopened. An overflowing extent fails before
loading its created mapping, whose returned failure cleans up explicitly. The
normal command suite and root tests (26 compile-fail cases) pass, as do Clippy,
formatting and Python syntax checks.

No mapping is adopted by name; random names and kernel create collision rejection
avoid removal of an existing device. Drop requests deferred removal, but that is
best-effort and is not a successful explicit teardown or quiescence proof. This
run does not separately qualify deferred-removal timing, late table-load faults,
setup deadlines or supervisor-death device cleanup. Full loop/verity/snapshot
assembly and bounded orchestration remain outstanding.

## RAM loop backing ownership

v79 is a preserved compile failure: the fixture attempted to return a workspace-
borrowing error as a static boxed error. No VM boot occurred. v80 booted and
invalidated the test's assumption that the kernel rejects an empty loop backing;
it does not. Production now rejects empty/non-4096-aligned backing explicitly.
v81 passes that check, attachment, actual loop writes, deferred detach with a
second holder, cleanup retry after releasing that holder, returned backing bytes,
file writes and reattachment. The normal suite also passes. Root tests, Clippy,
formatting and Python syntax checks pass.

LoopAttachment consumes Scratch and exposes no backing/raw handle while bound.
LOOP_CONFIGURE sets autoclear atomically. Initial selection/binding makes one
attempt: races fail with backing ownership retained, without a retry/sleep loop.
Explicit detach flushes, clears through a cloned descriptor (same open file
description), and requires ENXIO from status before returning scratch. A clear
request alone is not a detach receipt. Deferred cleanup keeps its original owner
for retry. This assumes no competing trusted supervisor reconfigures that owned
loop device; image commands must never receive loop/control access. Fixed setup
bounds, cache-backed read-only loop inputs, full snapshot assembly and crash/fault
qualification remain outstanding. Drop/autoclear is not a sealing receipt.

## Persistent snapshot ownership

v82/v83 passed ordinary I/O but are superseded: shared backing borrows permitted
multiple writers; exclusive borrows still released direct access when a wrapper
was dropped while its deferred kernel mapping could survive. Their source and
review findings are retained. Neither is qualified as the final ownership model.

v84 consumes LoopAttachment into SnapshotMapping. Setup and cleanup failures own
that attachment; explicit successful mapping removal returns it for subsequent
loop detachment. Drop requests deferred mapping removal and drops its backing
owner, so no scratch owner is returned while kernel references may survive.
The read-only origin remains borrowed; its kernel mapping also rejects immediate
removal while referenced. Two compile-fail checks report E0382 for direct backing
reuse and a second snapshot writer. The VM writes through the snapshot, verifies
EBUSY cleanup retention, removes it, detaches the loop, then reactivates the same
persistent store over fresh dm-zero and reads the bytes back. Normal suite and
code checks pass. This is not sealing: overflow/validity inspection, finalized
flush ordering, full chain integration and bounded orchestration remain required.

## Snapshot health and event-based death readiness

v85 verifies SnapshotMapping usage before/after writes and rejects kernel-invalid
status after exhausting its own 64KiB COW store. Write/flush reports failure; cleanup
still returns owned backing through the ordinary removal/detachment path. Usage is
an observation, not successful finalization or a sealed layer. Normal suite passes.

v86 passes replacing the two death-test sleep polls with inotify observation.
Review then found a content publication race, corrected with write-then-rename.
v87 preserves the resulting fixture failure: mv was missing inside the command
root. v88 adds that executable and dependencies, then passes supervisor death,
continued policy attachment and independent shutdown. Four host readiness tests
pass, including atomic rename publication; the added test source is archived
separately from the exact v88 booted source. No readiness depends on elapsed sleep.
All runs retain raw logs and artifact hashes; full sandbox qualification is false.

## Prepared cache-backed scutes

v89 adds three synthetic scute inputs: aligned raw, partial-block raw and zstd.
PreparedScute keeps aligned data cache-backed and generates a RAM verity tree;
other cases materialize only for decoding/padding. The guest confirms backing
selection, expected extents, trusted roots, rejection after same-length corruption,
and original inode retention after pathname replacement. The normal suite passes.
Host tests independently check emitted leaf/root hashes and failed tree writes.
This does not test read-time dm-verity protection or mountable scute metadata yet.
The fixture generator, source snapshots, exact initial cache blobs, logs and hashes
are retained in evidence/v89. No host mounts or device operations were performed.

## Owned dm-verity mappings

v90 records a failed test assumption: write-mode open may succeed even for a
read-only device. v91 tests write rejection instead and passes. PreparedScute is
consumed into VerityMapping, which retains both read-only loop bindings and their
backing. Reads return expected decoded/padded bytes for all three fixture formats.
A held block FD makes explicit close fail busy; releasing it allows cleanup retry.
A fresh activated mapping returns EIO after corruption of cache data before its
first read. No cached verified page is relied on for this negative test.

v91 includes exact booted source plus separately identified later compile-fail
lifetime checks, initial compiler-failure evidence, raw logs and verified hashes.
This qualifies these focused verity operations, not snapshot-chain assembly,
filesystem mounting, crash behavior of this new owner, or the complete sandbox.

## Owned read-only snapshot chains

v92 introduces a two-scute snapshot fixture and an authenticated but invalidated
second header. Both the successful stack and partially assembled failed stack
release all observed device-mapper names on explicit cleanup. v93 adds kernel
snapshot-health checks and an overlapping block: upper data replaces base data,
while a second block is inherited and an untouched block comes from dm-zero.
It also passes rejected writes and busy close followed by cleanup retry.

Snapshot headers are read through the owned dm-verity mapping after validating
the opened block-node identity. Shared devmap decoding supplies wire-format
checks; conglobate enforces the carapace chunk-size/validity requirements. Local
carapace reference sources and exact fixture blobs are included in v93 evidence.
Root lifetime checks pass; the post-boot documentation-only additions are labeled
separately. These are block-chain tests, not filesystem or build qualification.

## Writable snapshots over verified chains

v94 uses SnapshotOrigin's borrowed conversion from ReadOnlyChain. A fresh writable
snapshot inherits the two-scute view, then replaces an inherited block and writes
into a zero hole. The origin remains unchanged. Busy-close cleanup retains backing;
subsequent loop detachment/reattachment and snapshot reactivation preserve the new
bytes and untouched inherited data. The test rechecks the original view and cache
roots before complete cleanup. The normal suite and 34 compile-fail checks pass.
Exact source, fixtures, raw logs and artifact hashes are retained in evidence/v94.
This tests writable block layering, not mounted filesystems or sealed output.

## Mounted volume ownership

v95 mounts an ext4 filesystem on an owned writable snapshot in a private mount
namespace. The unformatted case first fails cleanly and returns its storage owner.
After fixture-only formatting, the test writes a file, creates/releases a borrowed
mount view, and forces EBUSY using an independent directory descriptor. Closing
that descriptor allows normal-unmount retry. Remount verifies persisted content;
final unmount and storage cleanup succeed. Empty attachment targets are removed.

Volume uses sealed mounted/unmounted states. Compile-fail checks prohibit unmount
with a live borrowed view and extracting a snapshot from a mounted volume. Exact
booted source, separately labeled later documentation tests, raw logs and verified
artifact hashes are in evidence/v95. These checks do not qualify production
formatting, command/policy handoff, setup deadlines, or final layer sealing.

## Supervisor volumes and private command views

v96 preserves a real namespace-construction failure: open_tree(CLONE) rejects an
attached root FD from another namespace. The archived kernel may_copy_tree excerpt
explains the EINVAL. v97 clones the detached view before namespace transition,
then assigns it to the command scope. v98 further idmaps and attaches that view,
verifying UID 100000 for filesystem UID 0 in both supported volume locations.

Supervisor-owned volumes create FilesystemPin directly from their root; its device
and inode match. Wrong-namespace cloning, pinning and cleanup are rejected, and
cleanup can retry after restoration. Remount persistence and the normal suite pass.
Exact booted sources, failed/intermediate logs and artifact hashes are preserved;
the later compile-fail lifetime test is identified separately. Protected-command
and supervisor-death qualification of the new volume path remain outstanding.

## Retained namespace descriptor for volume views

v99 replaces the volume fixture's namespace-path reopen with
`UserNamespace::idmap`. NamespacePin retains the original descriptor (or the
single verified descriptor during recovery). Both volume modes pass, including
actual ID mapping, policy pins and busy-unmount/remount handling. The protected
command path still needs its separate descriptor-transfer integration. Evidence
includes exact changed sources, host checks and the VM run; this is not full
sandbox qualification or an adherence pass.

## Supervised volume descriptor transfer

v100 adds `OWNED_VOLUME_SUPERVISED_TRANSFER_PASS`: a registered dedicated helper
receives the owned detached volume view and pinned user namespace over the
supervision channel, idmaps and attaches the view, and checks mapped root
ownership. The parent waits on readiness, cancels/reaps the helper and only then
releases its view. Existing volume cleanup and remount checks still pass. The
helper runs no image commands in this probe. Full protected-command integration
and aggregate Step/Volume lifetime enforcement remain outstanding. Host protocol
tests verify descriptor count, order and CLOEXEC; all evidence is retained.

## Step owns its command volume

v101/v102 add `STEP_RETAINS_VOLUME_THROUGH_QUIESCENCE_PASS`. The transfer helper now
receives admission through a Step that owns the mounted volume and bound policy.
The parent recovers the volume only after cancellation, reaping, descendant drain,
record cleanup and policy disablement. Existing unmount/remount checks pass.
The helper executes no image command in this probe. v101 retains intermediate
Clippy findings; v102 passes all checks with an explicit StepCompletion result.
Unfinished-step storage retention and two compile-time misuse cases are tested.
Volume fault injection and full image-command integration remain outstanding.

## Image commands on owned volumes

v103 executes the shared native sandbox script on the Step-owned snapshot volume.
v104 covers five outcomes: success, running-command timeout cancellation, shell
exit 37, cancellation before admission, and expired admission deadline. It checks
native stored UID 0 and exact SELinux label bytes for executed commands, and no
command output for startup-cancel cases. Every outcome recovers the volume after
descendant drain and policy disable, then exercises the existing unmount/remount
checks. The helper receives root/namespace descriptors rather than reopening the
historical source paths. Population remains fixture-only. Both runs are preserved;
owned-volume supervisor-death and broader setup-failure coverage remain open.

## Owned-volume setup faults and supervisor death

v105 adds detached setup error return and abrupt helper exit after attachment.
Both fail admission, leave command output absent, and recover the volume through
normal domain drain and policy cleanup. The full normal suite passes.

v106 runs `--supervisor-death` against the owned-volume path. The independent VM
controller kills conglobate, confirms a populated command domain, unmounts the
original volume and observes continued scoped labeling through the surviving
filesystem pin. It then powers off the VM. Serial evidence includes
`OWNED_VOLUME_AFTER_SUPERVISOR_DEATH_PASS` and the kernel shutdown message.
This proves these failure scenarios in the fixture, not exhaustive fault coverage
or recovery for another build in the same VM. Prior death evidence is preserved.

## Cleanup diagnostics preserve original failures

v107/v108 add mapping and volume diagnostic-history checks. A busy mapping remains
busy on retry; an unmount first fails EBUSY and then fails twice in the wrong
namespace. The failures retain original causes and ordered cleanup errors, and
still permit cleanup after the blocking condition is removed. v107 retains a
Clippy layout finding; v108 boxes history and passes all checks. Snapshot failures
expose mapping history too. Outer verity/chain cleanup propagation remains open.

## Verity and chain cleanup history

v109 extends retained diagnostics through verity and read-only-chain cleanup.
Busy retries preserve original errno and one entry per attempt; display output
contains no nested replay of child histories. Tests then close the reader and
complete cleanup. A host test verifies shared concrete error identity across
ownership boundaries. Root tests, Clippy and the normal VM suite pass; evidence
is archived. Command-error versus teardown-error orchestration remains separate
outstanding work.

## Fixed-budget step teardown

v110/v111 route all owned-volume command outcomes through `Step::teardown`.
Cancellation through policy disablement shares one fixed 30-second deadline;
failed retries retain that deadline and their cleanup phase. v110 preserves a
Clippy layout finding, fixed in v111. Root tests, both Clippy checks and VM suite
pass. Immediate expiry is tested without sleeping, and replacing the retry
budget is a compile-time error. Pin release and filesystem finalization remain
outside this coordinator and must join the overall budget before qualification.

## One cutoff through policy, pins and volume finalization

v112/v113 extend the fixed teardown deadline through policy cleanup, identity-pin
release and clean flush/unmount. Mounted storage no longer escapes the coordinator;
its completion converts to an unmounted volume under the same deadline. Every
owned-volume fixture forces a busy unmount and successfully retries after closing
the holder. v113 also blocks policy directory removal, checks retained identity
pins, and retries after removing the blocker. Root tests, Clippy and VM suites
pass. Source/evidence hashes are preserved. Setup budgeting and command-versus-
cleanup outcome handling remain outstanding.

## Restricted command preparation boundary

v114 separates final root/capability/seccomp preparation from image execution.
The helper obtains RestrictedCommand, then execution audits descriptors immediately
before spawning image code. A benign extra /dev/null FD is rejected after
restriction, with no image output; normal command cases pass. Host tests, Clippy
and VM checks pass. The source review also identified that outer-helper admission
still precedes inner sandbox preparation. A leaf readiness/start handshake and
correct setup-versus-execution clocks remain required; this test does not claim
to resolve that timing boundary.

## Restricted leaf readiness and explicit start

v115 runs owned-volume commands through ExecutionGate: the restricted inner helper
reports readiness, the outer helper then accepts admission, and explicit start
releases image execution. The gate closes before the final descriptor audit.
v116 passes the supervisor-death exercise on this path. Host tests cover malformed
packets, unexpected descriptor rights, cancellation, expiry and no start before
readiness. Exact VM sources and failures are preserved; the final redundant-borrow
Clippy correction is recorded separately. Production setup budgeting and command
outcome/timeout orchestration remain outstanding.

### M4 setup budget and supervisor-visible readiness (v117)

- The prior turn inspected synchronization without implementation progress. This
  turn adds SetupBudget (fixed 30 seconds), PreparedStep and RunningCommand. The
  trusted helper reports Prepared only after its restricted leaf reports ready.
  The supervisor consumes readiness before admission. Execution time begins before
  sending admission; acknowledgment latency, final FD audit and spawn count toward
  the finite command timeout. Late admission retains the running owner for cleanup.
- The owned-volume fixture creates its budget before preparation and uses the same
  cutoff for helper launch, volume transfer and readiness/admission. This is not
  yet complete production orchestration: intermediate acquisitions still need
  budget checks and potentially blocking operations need bounded execution.
- v117 normal disposable-VM suite passes, including timeout, setup failures,
  descriptor rejection and cleanup retry. Host tests cover readiness ordering,
  immediate expiry, wrong-phase packets and peer loss, with no sleeps. All host
  tests and 44 compile-fail tests pass; both Clippy checks pass. The initial large
  error-variant finding and source are preserved in setup-budget-clippy-v1; boxing
  its retained RunningCommand fixes it without losing cleanup authority.
- v117/source preserves exact booted source. Added host readiness tests are saved
  separately as post-run source. No adherence pass claimed; M4 streak remains zero.
  Production orchestration, primary command outcomes through cleanup failures and
  remaining M4 review are still outstanding. Later milestones remain pending.

### M4 primary command outcome and cleanup association (v118–v120)

- Previous turn progressed in c6688ef. The owned-volume fixture previously read
  command status from a marker file while teardown returned the outer trusted
  helper's exit status. These must not be conflated. Completion now travels on
  the private supervision channel as exact CBOR [1,7,bstr(four-byte big-endian
  Linux wait status)], with no ancillary descriptors. The helper reports before
  its own mount cleanup. The fixture no longer publishes a status marker file.
- RunningCommand::observe consumes the running owner and retains it alongside
  Exited or ObservationFailed, using the original execution cutoff and rejecting
  late observation. CommandResult consuming cleanup transitions retain that
  primary outcome even when cleanup returns a failure/retry owner. This is a
  diagnostic association, not a success receipt or proof of finalized storage.
- v118 passes ordinary completion/timeout/cleanup retry. v119 failed to compile
  a fixture &String pattern; exact source/logs are preserved, with no stale VM
  run. v120 corrects the pattern and combines shell exit 37 with policy cleanup
  failure and retry, proving 37 survives both transitions. Host tests preserve
  nonzero status and timeout through cleanup, validate exit/signal transport and
  reject nonterminal/reserved statuses, malformed lengths and extra descriptors.
  Host tests, 44 compile-fail cases, both Clippy checks and v120 VM pass. The
  initial Clippy items-after-tests finding is preserved in v118 and fixed by
  moving the implementation before tests. Source/evidence hashes are verified.
- Namespace init still normalizes shell signal termination to 128+signal. The
  completion transport preserves the reported namespace-init result; it does not
  recover original shell signal identity. Helper failure and command failure
  remain distinct observations. Finalization reports must continue carrying the
  primary outcome in the production orchestrator; fixture extraction currently
  separates them before its independent busy-unmount tests. Intermediate setup
  acquisition checks and full orchestrator integration remain outstanding.
  M4 adherence streak remains zero; no later milestone is advanced.
