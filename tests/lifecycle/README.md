# Maintained privileged lifecycle fixtures

This separate safe-Rust binary exercises production library owners in disposable
VMs. It is not a runtime dependency. Do not run privileged cases on the host.
The entrypoint refuses to proceed without the guest marker, including on trusted
helper re-exec. The marker is an accidental-use guard, not a security boundary.

Initial migration: `formatter-failures`, copied without assertion changes from
`validation/rewrite-command-integration/src/formatter_failures.rs` at 5ee7018.
It tests formatter deadline cancellation/reaping, exhausted writable-snapshot
rejection and explicit resource release. `FORMATTER_DROP=running` and `wait`
are terminal VM-disposal cases that check retained storage on abandonment.
The stalled formatter parks; synchronization does not use sleeps.

Compile with ordinary user privileges:

```
CARGO_BUILD_JOBS=2 TMPDIR=/var/tmp/conglobate-rewrite-tmp \
  cargo +stable check --manifest-path tests/lifecycle/Cargo.toml --locked
```

Compilation is not validation. Run the maintained disposable-VM runner with a
new output directory and a qualified local kernel/module tree:

```sh
python3 tests/lifecycle/run.py /var/tmp/NEW-LIFECYCLE-RUN \
  --kernel /path/to/vmlinuz --kernel-release RELEASE --module-root /
```

Use separate new directories with `--drop running` or `--drop wait` for the
terminal retention cases. The runner stages trusted host formatter tools and
the fixture, builds a disconnected native amd64/KVM guest, and preserves source,
input hashes, kernel identity, serial, raw status and assessment. Success requires
case assertions, status-disk exit zero and VM poweroff. It does not run BPF or
mount/device operations on the host. These trusted-formatter tests do not exercise
command labeling, so no LSM exception is needed for this subset.

The runner uses the maintained rootless GuestRuntime in tests/vm_runtime.py,
shared with the distro and seed runners. The original runner and
all historical results remain unchanged. Large run artifacts must be durably
archived before deletion; a temporary run path is not sufficient preservation.

The original harness and evidence remain intact. It currently fails to compile
against removed policy/pin APIs. Remaining command/network/bootstrap/publication
cases need explicit migration; none is considered retired or covered merely
because this subset compiles. Networking tests must use retained namespace
ownership without recreating policy-specific pins. Future tests must preserve
actual production ownership boundaries, not widen descriptor APIs for convenience.

The retired domain-record publication probe and parser tests are preserved under
`preservation/pre-domain-record-removal/`. The current implementation uses owned
cgroup descriptors without serialized recovery records.

`--case contained` exercises production `CommandDomain::launch` and contained
helper cleanup with trusted fixture processes: missing executable, peer exit
after identity exchange but before cgroup joining, and cancellation before
command admission with a live descendant. The child acknowledges readiness over
a pipe before the helper signals the supervisor over a private datagram socket.
Cancellation must remove the cgroup, reap the direct helper, drain at least one
adopted descendant and remove the protected record. Each case checks no remaining
children, child cgroups or records. No sleeps establish readiness. This is
containment-component coverage, not a mounted command-sandbox or publication test.

The contained case also aborts a trusted helper after descendant readiness. A
pidfd observes helper death before the fixture checks that its populated cgroup
and protected record remain owned. Explicit cancellation and drain must collect
the descendant, report SIGABRT for the helper and release those resources. This
exercises abrupt helper loss without Rust destructor execution.

`--case sandbox-namespace-failure` and `--case sandbox-mount-failure` migrate
pre-admission acquisition rollback from the historical volume-supervisor cases.
The former temporarily sets the disposable guest's user-namespace allowance to
zero and restores it before rollback; the latter deliberately leaves a dm-zero
snapshot unformatted. Both use the production CommandSandbox/Runtime owners,
retain original diagnostics, require clean resource directories and child drain,
and explicitly release the returned snapshot and runtime. No recipe runs and
these cases do not validate receiver admission, labels or mounted command teardown.

The `sandbox-launch-failure`, `sandbox-join-failure` and `sandbox-attached-exit`
cases format ext4 through the trusted production formatter and prepare a mounted
volume, mapped namespace and cgroup before injecting helper failure. Launch uses
a missing executable; join reuses the peer that exits after identity exchange.
Attached-exit receives the volume, prepares and attaches an idmapped root in a
private mount namespace, confirms mapped ownership, records its stage and exits
without destructors. The supervisor must reject setup and fully finalize rejected
storage. The fixture requires no children, attachment directories or cgroups,
then explicitly detaches the snapshot and closes the runtime. No recipe executes,
so this does not exercise labeling admission or post-admission receiver failure.

`--case external-network-input` checks the environment-owned namespace contract.
The trusted guest fixture creates a namespace, captures it through the production
NetworkNamespace type, deletes and recreates the pathname, and verifies that the
captured descriptor still identifies the original namespace. Releasing captured
handles must leave the replacement namespace intact for the environment to remove.
It also rejects missing inputs and the supervisor namespace and checks no fixture
children remain. This case does not validate command joining, routing, NAT or DNS;
the maintained distro network fixtures cover those integration paths separately.

The earlier `network-composite` allocation/teardown case is historical validation
of the superseded conglobate-owned network design. It is not an accepted option
in this maintained runner. Its preserved evidence does not qualify external
network provisioning or the current binary.

`--case sandbox-gate-failure` exercises the real mounted CommandReceiver after
supervisor admission. A trusted fixture child receives the ordinary gated offer,
shuts down its gate receive side before sending Ready, and waits on a fixture
socket for explicit release. Start must fail with BrokenPipe. The receiver's
primary-error diagnostic establishes entry into local cleanup; the supervisor
then verifies its control channel remains open while the child is held alive.
After release the receiver must return Cleaned, reap the child, close its view,
and exit successfully. The supervisor still treats the absent command completion
as failure and finalizes rejected storage. No image command or labeling executes.
An isolated early-channel-close mutant fails the liveness assertion; no production
API or descriptor access was widened for this test. No sleeps establish order.

`--case sandbox-blocked-io` suspends only its owned snapshot in the disposable VM
and releases a trusted filesystem writer. It observes task state D before waiting
for command timeout, then requires the production fixed 30-second teardown to
fail while retaining mounted storage and the populated domain. Retry must fail
without renewing the budget. This terminal case claims no clean finalization.
`--case sandbox-supervisor-death` aborts the owning library supervisor with the
same blocked writer; independent guest init must record status 134 and power off.
These test the disposal contract, not a specific production appliance controller.

Guest init flushes only its separate result block device before forced poweroff.
A global sync would wait on deliberately suspended storage: the first blocked-I/O
run demonstrated that observer bug and is preserved as a failure. No privileged
host setup or synchronization sleeps are used. D-state observation checks the
actual task state under a deadline, yielding between observations.

The gate-failure case also holds an ordinary directory descriptor on the mounted
ext4 volume during finalization. Unmount must return ResourceBusy, retaining the
mount and snapshot independently of the already-retired command-domain record.
Closing the holder permits retry under the original deadline. Both transitions
must preserve the original failed command observation, despite successful helper
exit. Final resource checks require no mounts, domains, records or children.
This covers busy-unmount recovery, not filesystem writeback errors.

`--case sandbox-writeback` uses an 8 MiB writable snapshot beneath its 16 MiB
ext4 volume. After actual command-observation failure and process teardown,
trusted fixture filesystem writes exhaust the COW. The fixture requires EIO or
EROFS plus an independent dm snapshot Invalid status. Production finalization
and retry must fail with storage diagnostics and preserve the original command
failure; no finalized storage is returned. Independent init disposes of the VM.
Observed runs reject at syncfs after ext4 journal abort; they do not exercise
the later post-unmount snapshot-validation branch or an image-authored writer.

`--case cache-storage` migrates the historical synthetic scute preparation
checks. Aligned uncompressed blobs remain cache-backed; compressed and partial
blocks materialize in RAM. It checks verified read-only access, busy cleanup
retention/diagnostics, rejection of a modified cache block after activation
before first read, failed re-preparation, and inode retention across path
replacement. Only the guest's private synthetic cache is modified. This is
storage-component evidence, not a filesystem, full-chain or boot qualification.

`--case sandbox-busy-execution` tests the high-level execution acceptance API.
A trusted fixture helper reports successful completion; a held directory makes
finalization fail with EBUSY. After releasing the holder, retry must return a
failure owner with the original cleanup error and only rejected storage for
disposal. Final resource checks require complete release. No image code or
labeling executes in this focused test.

`--case clone-filter` compiles the preserved benign C clone fixture and executes
it only inside the disconnected disposable VM. The Rust driver applies the
current production `CommandSyscalls` filter between control and filtered arms.
Both retain namespace authority and set no_new_privs; each child does nothing
and is reaped. Ordinary clone must succeed in both arms. All seven tested legacy
namespace flags must succeed before filtering and return EPERM afterward. The
runner checks every case plus status and poweroff. This is filter-only amd64
coverage, not command capability-drop composition or Arm qualification. The
legacy CLONE_NEWTIME bit overlaps the exit-signal byte and is not covered.

`--case sandbox-held-cleanup` uses a trusted attached helper to report shell
success while its view remains mounted behind a datagram gate. The supervisor
observes success, deliberately waits for the original command deadline to expire,
then releases cleanup and requires clean teardown/finalization. The timer wait
is the assertion target; readiness is synchronized by the status and datagram
protocol, never by sleeping. This is a lifecycle test with simulated shell status.

`--case sandbox-missing-cleanup` has the helper report shell success and exit
without sending a cleanup acknowledgment. Production `Step::execute` must return
failure; cleanup retry retains diagnostics and recovers only rejected storage.
Both cases check all resources and child processes are released before poweroff.

`--case sandbox-cleanup-timeout` reports simulated shell success and parks the
trusted receiver with its view still attached. With a one-second command budget,
production execution must instead fail at the fixed 30-second cleanup deadline,
retain the successful primary outcome and resource owners, and refuse storage
recovery after an immediate retry. The fixture retains its failure owner and
requires independent VM poweroff; it does not claim clean teardown or resource
reuse. A parked helper is not a kernel uninterruptible-I/O test.

`--case sandbox-blocked-descendant` runs the production receiver and namespace
init with a minimal Bash/sync command root. The disposable VM loads the existing
external exact-label policy fixture. A child stops itself before a benign file
write; the trusted parent confirms its stopped state, suspends only the owned
snapshot, resumes the child, and confirms D state. The parent then signals the
shell's exit trap. Shell success must arrive within the execution budget while
the child remains blocked. Cleanup must fail at the fixed 30-second cutoff,
retain storage and recovery records, and reject an immediate retry without
extending the deadline. The independent guest init powers off the VM. No device
or control authority is passed to the command, no synchronization sleeps are
used, and no successful cleanup, artifact publication or full sandbox
qualification is claimed. This test supplements the pre-completion
`sandbox-blocked-io` case; it does not replace it.

`--case sandbox-delayed-teardown` observes simulated shell success from a trusted
receiver parked before cleanup. It deliberately holds the observed owner until
its cleanup deadline expires, then requires both teardown and retry to fail in
under one second with storage and recovery records retained. Waiting for the
deadline is the assertion target, not synchronization. Independent VM disposal
ends this terminal case. This tests the split observation/teardown API rather
than an image command or successful finalization.

`--case sandbox-unmapped-owner` creates ordinary ext4 files with UID/GID 65535
and 65536 before production sandbox preparation. Preparation must reject the
unsupported ID before launching a receiver; rollback must release its resources.
A supervisor-side remount checks that original disk IDs remain unchanged.
`--case sandbox-unmapped-acl` instead uses an inode owned by root with a named
access ACL user entry for ID 65536 and requires the same preparation rejection.
`--case sandbox-unmapped-default-group` uses a root-owned directory with a
default ACL naming group 65536, and requires a GID-specific rejection before
receiver launch followed by complete rollback.
These cases execute no image command and do not qualify the final supported ID range.
The earlier overflow-ID characterization remains recoverable in
preservation/m7-review/unmapped-owner-v1/, including its source snapshot.

## Hosted lifecycle CI

The Disposable VM lifecycle workflow builds the maintained fixture with pinned
Rust 1.94.0 and an installed Ubuntu generic kernel. It runs cache-storage,
contained, runtime-scope and clone-filter in separate native amd64 KVM guests.
The job requires accessible KVM; missing virtualization is a failure, not a skip.
Package installation and KVM permissions are CI host preparation; privileged
filesystem, namespace and policy assertions execute only in the guest. No
recipe-supplied programs are run directly on the runner host.

The runner honors RUSTUP_TOOLCHAIN (default stable), CARGO_BUILD_JOBS and TMPDIR.
It builds offline from the locked fixture dependencies after the workflow fetch.
CI uploads raw logs/status, assessments, input records, source archive and guest
kernel/initramfs even after failure. Artifact retention is 14 days, not a durable
M8 archive. This job covers lifecycle components, not full distro builds/boots,
arm64, confidential VMs, or the environment-loaded labeling policy.

For mount compatibility diagnosis, --trace-mounts stages the trusted host strace
executable and records selected mount/pidfd/statx syscalls in guest serial output.
Tracing runs only inside the disposable guest. Treat an instrumented result as
diagnostic evidence; repeat ordinary execution when qualifying a fix.

`--case runtime-private-namespace` runs the runtime lifecycle checks in a fresh
private supervisor mount namespace. `--case runtime-changed-namespace` verifies
that changing namespaces after resource acquisition rejects cleanup and leaves
resources for guest disposal. Neither case executes untrusted image commands.
