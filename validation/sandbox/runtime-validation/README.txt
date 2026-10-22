Sandbox runtime validation pass — 2026-10-21
==========================================

Purpose and decision boundary
-----------------------------
Exercise every candidate from ../runtime-comparison/REPORT.txt against the same
benign filesystem-worker/command topology. This is a backend-selection experiment,
not production sandbox qualification or a general security ranking of runtimes.
No conglobate production Rust code was changed for these experiments.

The fixture runs ONLY inside a disposable QEMU guest: Fedora kernel
7.2.10-200.fc44.x86_64, SELinux disabled, 6 GiB RAM, two vCPUs, no network device,
no disk device, no shared directory. KVM accelerates this test VM; the tested
sandbox runtimes do not use nested virtualization. The guest's initial root
supervisor controls only disposable guest objects. All images are RAM-backed.
No hostile filesystem, exploit, or attack against host resources is involved.

Tested versions
---------------
- Bubblewrap 0.12.0-1.fc44.
- crun 1.28-1.fc44, explicitly disabling its cgroup manager.
- runc 1.5.2-3.fc44, extracted RPM with libpathrs 0.2.6.
- Youki CLI v0.7.0 upstream x86_64-musl release.
- libcontainer 0.7.0 at cb3214c2204a03f1b99ae7c5870138439fd84345, embedded in
  a small #![forbid(unsafe_code)] single-threaded Rust helper. Cargo.lock is kept.
  Only the v2 feature is enabled; this tests cgroup-v2 operation, not a cgroup-free
  or multithreaded application integration.
- NsJail source 4ff54a6e0d5b65a0e1633d6e2fd1425ba6c882ce, including its pinned
  Kafel submodule. Built locally with extracted Fedora development packages.
- systemd services and systemd-nspawn 259.9-1.fc44, dbus-broker 37-8.fc44.
- util-linux unshare/nsenter/setpriv 2.41.5-1.fc44, plus our explicit test adapter.

The worker and patched libfuse are inherited unchanged from the recorded
../two-sandboxes/ and ../worker-placement-fix/ fixtures. This pass changes no
worker/library patch. Input hashes identify the actual binaries; an upstream
source review is not interchangeable with testing an installed release.

Common fixture and acceptance checks
------------------------------------
1. A trusted helper creates the command's user namespace with IDs 0..65535 mapped
   to guest IDs 100000..165535. It mounts FUSE with allow_other, nosuid, nodev and
   actual default_permissions. It passes the connection FD to the supervisor.
2. A distinct worker user namespace maps IDs to 200000..265535 (Bubblewrap uses
   its single-identity worker mapping). The worker receives its image file and
   FUSE FD, zero effective capabilities, no_new_privs, private PID/mount/network/
   IPC/UTS namespaces, and a read-only runtime. It cannot see the context or the
   supervisor's sentinel paths. It runs the existing patched fuse2fs.
3. After starting the worker, the helper sends a detached mount FD using
   open_tree; the supervisor attaches it using move_mount. The command receives
   that mount at /image and joins its owning user namespace. The worker and
   command remain separate sandboxes. Numeric mappings alone are insufficient.
4. The command asserts FUSE_SUPER_MAGIC, exact effective capabilities, namespace
   identities distinct from supervisor and worker, no_new_privs, no unexpected
   handles, no image backing file or supervisor/context paths, and no non-loopback IPv4 route.
   The Python instrument opens one read-only libffi FD; that specific runtime
   FD is allowed, not confused with a supervisor control FD.
5. Metadata operations cover ownership, ACL access/default inheritance, mode and
   supplementary-group setgid behavior, foreign SELinux labels, and file caps.
   A child explicitly removes SYS_ADMIN and verifies the same label write fails
   with EPERM. It is not sufficient merely to assert that requested argv omitted
   a capability. Normal completion unmounts and waits for worker exit, then runs
   e2fsck -fn. Persistence/remount coverage from older tests is separate evidence.
6. A uniform trusted payload adapter installs a small seccomp probe that rejects
   selected namespace/mount/inspection/module/keyring syscalls and checks denial
   of unshare(0). It is installed before filesystem operations or worker exec.
   This tests compatibility with that filter, NOT each runtime's native seccomp
   configuration or a complete production syscall/ioctl policy.
7. Each successful backend runs normal exit, exit42, descendant cleanup, command
   launcher death, actual worker death, an explicit termination request, and
   mount-helper death. The latter four are observations, not automatic PASSes.
   After a bounded observation the outer harness cleans up its own processes and
   mounts. It checks no live recorded namespace processes or FUSE connections
   remain. Cleanup by the harness is not credited to the runtime.

The 'timeout' case injects SIGKILL after a readiness marker. It validates the
termination path, not a five-minute timer, a runtime's native timer, or the future
conglobate recipe timer. Launcher death kills the client process, not the outer
coordinator. It must not be described as full supervisor-death qualification.
A half-second grace avoids calling a transient exit/reaping race a process leak.

Important correction to earlier evidence
---------------------------------------
The original two-sandboxes command used --cap-add without --cap-drop ALL. A
repeat measured CapEff=000001ffffffffff. Its claim that command CAP_SYS_ADMIN
was absent was wrong. The final explicit allowlist measures 00000000802401db;
the negative control proves removing SYS_ADMIN prevents foreign SELinux-label
writes on the actual FUSE filesystem in this configuration. SANDBOX-DESIGN.txt
and the historical README are corrected. Initial-namespace root capability is
still absent. The increased command authority within its user namespace is a
real consideration for the production syscall/ioctl policy.

The explicit test list is CHOWN, DAC_OVERRIDE, FOWNER, FSETID, SETGID, SETUID,
SETPCAP, SYS_CHROOT, SYS_ADMIN and SETFCAP. This pass establishes compatibility,
not that every listed capability is necessary or the list is safe for release.

Adapters and practical implications
-----------------------------------
Bubblewrap: trusted pre-entry into the owning user namespace; explicit drop-ALL
then add-list. Pass the FUSE connection through stdin during worker setup, then
move it to FD3 and restore null stdin. --die-with-parent is not a substitute for
coordinating a separately held FUSE mount and the other sandbox's lifetime.

crun/runc/Youki: prepare an OCI bundle around the existing mounts; no OCI pull,
Docker daemon, or image conversion. Join the owning user namespace through the
OCI namespace path; preserve the worker FD directly. OCI state and, except crun
in this fixture, the cgroup-v2 hierarchy need lifecycle management.

libcontainer: the safe Rust helper actually creates, starts, waits for, and
deletes containers. It uses child-subreaper behavior and a single-threaded process.
This removes an external CLI protocol, not the subprocess/process-state problem.
Library-internal unsafe implementation is not forbidden by our safe caller rule.

NsJail: trusted pre-entry via nsenter for the command; direct FD preservation for
the worker. Its internal timer is disabled so all candidates receive the same
external lifecycle events. This is not validation of NsJail's time_limit policy.

systemd services: a real guest system manager and D-Bus are present. Use
UserNamespacePath, User=0/Group=0, private namespaces, RootDirectory/bind mounts,
capability controls, no_new_privs and KillMode=control-group. The explicit user
setting matters: simply joining a namespace while retaining an unmapped outer
UID produced overflow UID65534. Using outer numeric UID200000 inside the joined
namespace instead failed with 217/USER. Corrected worker setup succeeds, while
command setup remains unresolved (226/NAMESPACE). Delegating namespaces also
failed in this fixture. This is an integration blocker here, not proof systemd
cannot implement the design. Its manager dependency is independently a tradeoff.

nspawn: the separately mapped worker starts successfully. Command pre-entry
requires additional private /run and mount-namespace construction for launcher
bookkeeping; even with those adapters and a private PID namespace, its early
/proc mount fails EPERM. Merely supplying the same numeric --private-users map
would create a different user namespace and evade the actual requirement. We did
not weaken the test that way. Whether another nspawn integration can satisfy the
contract remains unresolved; it is not the most direct candidate demonstrated.

unshare: works only with our custom mount/bind/remount, pivot_root, old-root
removal, credential/capability and process management adapter. The result is
not evidence that prepending 'unshare' supplies a sandbox policy.

Harness corrections and interpretation
--------------------------------------
Early attempts exposed fixture problems: missing initramfs runtime dependencies,
minimal systemd/D-Bus startup ordering, unmapped UID handling, device ownership,
and mount handoff. In particular, mount(8) canonicalization of a proc-root path
could expose the supervisor's underlying tmpfs instead of the helper's FUSE
mount. These runs' label failures are NOT runtime incompatibility evidence.
Direct bind of a mount in another mount namespace also failed. The final FD
handoff and statfs assertion remove that ambiguity. Another early harness bug
left exported mounts pinned after helper death; final cleanup unmounts regardless
of helper liveness. Earlier logs are preserved, not counted as final outcomes. A later route check
was corrected to accept an empty route table or loopback-only routes, rather
than requiring exactly one header line; no external interface route is allowed.

Reproduction and evidence
-------------------------
run.py packages and runs the disposable guest. It requires the prior fixture's
root, kernel and command.json plus the downloaded/extracted runtimes; it is not
a standalone distribution installer. Parameters --seed, --downloads and --sources
replace the default local research paths. The final inputs.json hashes regular
files, symlinks.json records link targets, and command.json records guest launch.
Reproduction must supply matching inputs, not assume a future host RPM is equal.

Build the library helper with:
  CARGO_TARGET_DIR=/var/tmp/conglobate-runtime-validation-downloads/libcontainer-target \
    cargo build --locked --manifest-path validation/sandbox/runtime-validation/libcontainer/Cargo.toml

Build NsJail in the pinned checkout after initializing its pinned submodule.
Use extracted RPM sysroot/usr/bin on PATH, sysroot/usr/lib64 on LD_LIBRARY_PATH,
and sysroot/usr/lib64/pkgconfig plus PKG_CONFIG_SYSROOT_DIR for pkg-config.
The saved build log and RPM hashes identify that build's dependency set.

Then, from the repository:
  python3 validation/sandbox/runtime-validation/run.py /var/tmp/NEW-DIRECTORY --run

RESULTS.txt gives the comparison and recommendation. Raw evidence is in
../reproduced/runtime-validation/.
Guest completion means all requested candidates were attempted and classified;
it does not mean every candidate passed. verdict.json deliberately sets
production_qualified=false. The runnable Python fixture uses ctypes/syscalls
inside the guest; it is test scaffolding, not proposed production Rust.

Still outside this selection pass
---------------------------------
- Actual production carapace/block-device storage, full writable OS root and
  bootstrap/firmware phases; this fixture uses a RAM-backed ext4 file at /image.
- Online NAT, supplied DNS configuration, resolver symlink handling and inbound
  policy. Only disconnected private networks were used in this pass.
- Complete seccomp/ioctl/clone/descriptor policy and adversarial security review.
- Exhaustive cancellation/OOM/blocked-I/O/resource-limit and supervisor-death
  cases; previous probes cannot automatically qualify a different backend.
- Native arm64 and real Intel/AMD/Arm confidential-computing environments.
- Distro builds and a performance comparison. These short functional probes do
  not establish production throughput or startup-time rankings.
