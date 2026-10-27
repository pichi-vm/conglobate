# Tests

Run commands from the repository root. Privileged tests run only inside
disposable VMs; rootless checks are documented in [README.md](../README.md).
Use repository-local scratch output directories and remove generated artifacts
when finished. Each runner requires a new output directory.

## Trusted guest labeling environment

The files in `tests/environment/` provision the fixed exact-name `security.selinux` exception in
an owned disposable VM. They are not linked into conglobate, not a recipe input,
and must never be loaded on the development host. The loader requires the guest
marker, loads once and pins the link for the VM lifetime. There are no subject or
filesystem registrations. Ordinary inode, mount, ownership and ACL checks remain.

The production execution environment must supply the equivalent contract.
The fixture requires clang with BPF support, a C compiler and libbpf development
files. `tests/distro/label_environment.py` obtains flags from pkg-config. Explicit
`CONGLOBATE_TEST_BPF_CFLAGS` and `CONGLOBATE_TEST_BPF_LIBS` override those flags
for an externally prepared sysroot; they contain shell-style argument lists,
parsed as arguments and never executed as shell code. Multiarch UAPI headers are
found using the C compiler's `-print-multiarch` result when present.

## Lifecycle harness

This separate safe-Rust binary exercises the binary's private production owners in disposable
VMs. It is not a runtime dependency. Do not run privileged cases on the host.
The entrypoint refuses to proceed without the guest marker, including on trusted
helper re-exec. The marker is an accidental-use guard, not a security boundary.

`formatter-failures` tests formatter deadline cancellation/reaping, exhausted writable-snapshot
rejection and explicit resource release. `FORMATTER_DROP=running`
is a terminal VM-disposal case that checks retained storage on abandonment.
A failed wait now performs its own bounded cancellation and cleanup.
The stalled formatter parks; synchronization does not use sleeps.

Compile with ordinary user privileges:

```
CARGO_BUILD_JOBS=2 \
  cargo +stable check --locked --features test-fixtures --bin conglobate-lifecycle-tests
```

Compilation is not validation. Run the maintained disposable-VM runner with a
new output directory and a qualified local kernel/module tree:

```sh
python3 tests/lifecycle/run.py .scratch/NEW-LIFECYCLE-RUN \
  --kernel /path/to/vmlinuz --kernel-release RELEASE --module-root /
```

Use separate new directories with `--drop running` for the
terminal retention cases. The runner stages formatter tooling needed by the
current binary (transitional; product contract is the internal formatter map)
and the fixture, builds a disconnected native amd64/KVM guest, and preserves source,
input hashes, kernel identity, serial, raw status and assessment. Success requires
case assertions, status-disk exit zero and VM poweroff. It does not run BPF or
mount/device operations on the host. These trusted-formatter tests do not exercise
command labeling, so no LSM exception is needed for this subset.

The runner uses the maintained rootless GuestRuntime in tests/vm_runtime.py,
shared with the distro and seed runners. Cases use the current production
ownership boundaries. Remove generated run artifacts when they are no longer
needed; they are not repository inputs.

`--case contained` exercises production `CommandDomain::launch` and contained
helper cleanup with trusted fixture processes: missing executable, peer exit
after identity exchange but before cgroup joining, and cancellation before
command admission with a live descendant. The child acknowledges readiness over
a pipe before the helper signals the supervisor over a private datagram socket.
Cancellation must remove the cgroup, reap the direct helper, drain at least one
adopted descendant. Each case checks no remaining children or child cgroups
and verifies the protected lease directory stays empty; no records are written. No sleeps establish readiness. This is
containment-component coverage, not a mounted command-sandbox or publication test.

The contained case also aborts a trusted helper after descendant readiness. A
pidfd observes helper death before the fixture checks that its cgroup remains
populated and its protected lease directory remains empty. Explicit cancellation and drain must collect
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
`--case sandbox-supervisor-death` aborts the owning supervisor with the
same blocked writer; independent guest init must record status 134 and power off.
These test the disposal contract, not a specific production appliance controller.

Guest init flushes only its separate result block device before forced poweroff.
A global sync would wait on deliberately suspended storage: the first blocked-I/O
run demonstrated that observer bug and is preserved as a failure. No privileged
host setup or synchronization sleeps are used. D-state observation checks the
actual task state under a deadline, yielding between observations.

The gate-failure case also holds an ordinary directory descriptor on the mounted
ext4 volume during finalization. Unmount must return ResourceBusy, retaining the
mount and snapshot independently of the already-drained command domain.
Closing the holder does not turn the failed build into success. The original
failed command observation and ResourceBusy cause remain inspectable, and the
mounted storage stays retained until VM disposal. This covers terminal busy
unmount, not filesystem writeback errors.

`--case sandbox-writeback` uses an 8 MiB writable snapshot beneath its 16 MiB
ext4 volume. After actual command-observation failure and process teardown,
trusted fixture filesystem writes exhaust the COW. The fixture requires EIO or
EROFS plus an independent dm snapshot Invalid status. Production finalization
must fail with storage diagnostics and preserve the original command
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
finalization fail with EBUSY. Releasing the holder does not return accepted or
reusable storage. The test requires the original successful command observation,
the cleanup error and a retained mount, then disposes of the VM. No image code
or labeling executes in this focused test.

`--case clone-filter` compiles the preserved benign C clone fixture and executes
it only inside the disconnected disposable VM. The Rust driver applies the
current production `CommandSyscalls` filter between control and filtered arms.
Both retain namespace authority and set no_new_privs; each child does nothing
and is reaped. Ordinary clone must succeed in both arms. All seven tested legacy
namespace flags must succeed before filtering and return EPERM afterward. The
runner checks every case plus status and poweroff. This is filter-only coverage on both CI architectures, not command
capability-drop composition. The
legacy CLONE_NEWTIME bit overlaps the exit-signal byte and is not covered.

`--case sandbox-held-cleanup` uses a trusted attached helper to report shell
success while its view remains mounted behind a datagram gate. The supervisor
observes success, deliberately waits for the original command deadline to expire,
then releases cleanup and requires clean teardown/finalization. The timer wait
is the assertion target; readiness is synchronized by the status and datagram
protocol, never by sleeping. This is a lifecycle test with simulated shell status.

`--case sandbox-missing-cleanup` has the helper report shell success and exit
without sending a cleanup acknowledgment. Production `Step::execute` must return
failure after attempting cancellation and descendant drain. Missing protocol
confirmation never produces finalized storage, even when the helper exited.
The held-cleanup case checks full release; missing cleanup is terminal and retains
the mount for VM disposal.

`--case sandbox-cleanup-timeout` reports simulated shell success and parks the
trusted receiver with its view still attached. With a one-second command budget,
production execution must instead fail at the fixed 30-second cleanup deadline,
retain the successful primary outcome and resource owners. The fixture
requires independent VM poweroff; it does not claim clean teardown or resource
reuse. A parked helper is not a kernel uninterruptible-I/O test.

`--case sandbox-blocked-descendant` runs the production receiver and namespace
init with a minimal Bash/sync command root. The disposable VM loads the existing
external exact-label policy fixture. A child stops itself before a benign file
write; the trusted parent confirms its stopped state, suspends only the owned
snapshot, resumes the child, and confirms D state. The parent then signals the
shell's exit trap. Shell success must arrive within the execution budget while
the child remains blocked. Cleanup must fail at the fixed 30-second cutoff,
retain storage without extending the deadline. No recovery records are written. The independent guest init powers off the VM. No device
or control authority is passed to the command, no synchronization sleeps are
used, and no successful cleanup, artifact publication or full sandbox
qualification is claimed. This test supplements the pre-completion
`sandbox-blocked-io` case; it does not replace it.

`--case sandbox-delayed-teardown` observes simulated shell success from a trusted
receiver parked before cleanup. It deliberately holds the observed owner until
its cleanup deadline expires, then requires teardown to fail in under one second
with storage retained and no recovery records. Waiting for the
deadline is the assertion target, not synchronization. Independent VM disposal
ends this terminal case. This tests the split observation/teardown API rather
than an image command or successful finalization.

`--case sandbox-unmapped-owner` creates ordinary ext4 files with UID/GID 65535
and 65536 before production sandbox preparation. Preparation must reject the
unsupported ID before launching a receiver; rollback must release its resources.
The failed preparation returns no volume for remount/reuse. The test checks ID admission
and resource release.
`--case sandbox-unmapped-acl` instead uses an inode owned by root with a named
access ACL user entry for ID 65536 and requires the same preparation rejection.
`--case sandbox-unmapped-default-group` uses a root-owned directory with a
default ACL naming group 65536, and requires a GID-specific rejection before
receiver launch followed by complete rollback.
These cases execute no image command and do not qualify the final supported ID range.

### Hosted lifecycle CI

The workflow builds with pinned Rust 1.94.0 on native amd64 and Arm64 runners.
It runs cache-storage, contained, runtime-scope, runtime-private-namespace,
runtime-changed-namespace, clone-filter and pty-filter. amd64 requires KVM;
Arm64 uses KVM if available, otherwise explicitly recorded TCG. TCG is a CI
fixture, not a production sandbox design or confidential-hardware claim.
Privileged assertions run only inside disposable guests.

The runner honors RUSTUP_TOOLCHAIN (default stable) and CARGO_BUILD_JOBS and
builds offline from locked dependencies after the workflow fetch. CI retains
logs, status, assessments and input/source records for 14 days; durable acceptance
evidence is recorded separately. This workflow does not run full distro builds
or qualify confidential VMs.

`--case oci-copy --oras /path/to/oras` preserves the original third-party OCI
interoperability assertion: a committed pass-through build publishes a synthetic
scute chain, ORAS copies its manifest/config/layer graph, and the fixture compares
the exact blob set and every byte. Inventory stays outside the OCI graph. amd64
CI uses checksum-pinned ORAS 1.3.5. This case runs as root in the disposable guest;
it does not require unprivileged user namespaces on the runner host.

For mount compatibility diagnosis, --trace-mounts stages the trusted host strace
executable and records selected mount/pidfd/statx syscalls in guest serial output.
Tracing runs only inside the disposable guest. Treat an instrumented result as
diagnostic evidence; repeat ordinary execution when qualifying a fix.

`--case runtime-private-namespace` runs the runtime lifecycle checks in a fresh
private supervisor mount namespace. `--case runtime-changed-namespace` verifies
that changing namespaces after resource acquisition rejects cleanup and leaves
resources for guest disposal. Neither case executes untrusted image commands.

## Distro harness and inputs
Paths such as `context/recipe.yaml` below are relative to `tests/distro/`.


`context/recipe.yaml` is a small offline CLI regression using the Fedora seed.
The runner commits `message.txt` with the recipe; COPY installs it into the image,
and a command checks its bytes and the absence of context mounts. The runner then
requires successful publication/export and verifies exported blob digests. This
checks the committed-context pipeline, not distro boot or full qualification.

`context/lock.yaml` pins `qualification.local/fedora:43` to a tiny fixture index.
The host harness seeds that exact index blob. It is not a Fedora GHCR digest.
`run.py` still writes its own `images.lock` from `--pin`.

`pipeline/recipe.yaml` exercises bootstrap, three carapace directives and firmware
with an explicit tool image in one offline build. Supply the Fedora seed and
`--firmware-pmi` / `--firmware-dtb` using its existing pair. The bootstrap installs
only a shell, rm and their shared libraries into a small new root. Later steps
check persistence across command/COPY boundaries and separation of tool roots.
Run `python3 tests/distro/pipeline/verify.py RUN_DIRECTORY` afterward (requires
`cbor2` and `fdtget`). It requires exactly two produced scutes, checks the exported
blob graph and canonical inventory, and reads `/chosen/bootargs` to verify the
final corium root. It does not qualify booting or all stage/failure combinations.

`prepare.py` copies an existing local artifact manifest and its components into
a new OCI image-layout directory, verifies their SHA-256 digests and sizes, and
creates a pinned architecture index for conglobate's lock file. It never downloads
artifacts. Run it without privileges:

```sh
python3 tests/distro/prepare.py \
  --source-cache /path/to/cache \
  --manifest sha256:MANIFEST_DIGEST \
  --architecture amd64 \
  --output /path/to/new/cache
```

The output must not exist. Successful output is made read-only and `index.pin`
contains the index digest. Failed attempts remain available for inspection.
The supplied architecture describes the existing artifact; this script does not
inspect executable machine types or convert architectures.

This is fixture preparation, not distro qualification. Conglobate must still
verify the carapace chain, execute a real distro recipe, and produce an image
that boots. See DESIGN.md section 12 for validation scope and limits.

The initial real Fedora recipe is `fedora/recipe.yaml`. It uses the existing
local Fedora carapace, runs RPM/DNF, installs SELinux tooling and targeted policy,
and labels the resulting root with that policy. It inherits firmware. A completed
build alone does not qualify booting or enforcing the installed policy.

`run.py` builds a fresh disposable amd64/KVM VM using the same trusted runtime
constructor as the integration tests. It adds outbound QEMU user networking for
package transactions; the existing regression VM remains disconnected. Supply
`--kernel`, `--kernel-release`, `--module-root`, `--cache`, `--pin`, and `--recipe`
plus a new output directory. The current lock generation uses the Fedora recipe's
`qualification.local/fedora:43` reference. All conglobate artifacts remain local;
network downloads occur only in the explicitly network-enabled DNF command.

The supervisor exports the finished OCI layout to a dedicated raw virtual disk,
which commands cannot access. The host extracts it with Python's data-only tar
filter and checks all blob digests. Source snapshots, committed context bundle,
input hashes, console logs and verdicts remain under the run directory, including
failures. Workspaces remain RAM-backed. No host mount, device-mapper operation or
BPF load is performed. The shared runtime constructor is maintained in tests/vm_runtime.py; this
runner covers a build, with boot checked separately.

`boot.py ARTIFACT NEW_OUTPUT_DIRECTORY` is an optional pichi/dillo interoperability
check of the produced image. It uses an isolated cache and loads a disposable
copy because the installed pichi loader consumes input blob paths. Neither tool
is a conglobate runtime dependency, and this does not replace the ORAS CI checks.
The default required marker comes from the Fedora recipe's oneshot service, which
checks enforcing mode, the target file's expected SELinux label and root ownership.
The harness terminates the VM after the marker; it does not claim guest shutdown.
A different `--expect` marker narrows the claim accordingly (e.g. login alone does
not execute the file-label assertions). Console logs and launcher identities are
preserved. Waiting uses pipe readiness, not synchronization sleeps.

Bootstrap no longer accepts `--formatter-runtime`. Conglobate selects the ext4
formatter through an internal filesystem-name map and discovers host `mke2fs`
(+ `ldd` deps) itself. `GuestRuntime.TOOLS` still stages `mke2fs` into disposable
VM roots so that discovery succeeds inside the guest.

TRANSITIONAL: lifecycle formatter-failure fixtures still stage
`/formatter-runtime-files` and `/formatter-stall-files` lists for injection;
those are test-only file lists, not a product CLI flag, and should go away when
failure fixtures move onto image-resident tooling.

`seed.py` prepares an initial tool filesystem outside conglobate from an explicitly
SHA-256-pinned complete rootfs tar archive. It rejects layered whiteouts and member
path traversal, formats/extracts only in a disconnected disposable VM, checks
passwd/shadow numeric ownership and modes, unmounts, and runs e2fsck. Its sparse
64-GiB filesystem can then be converted into an initial carapace externally. This
is initial seed preparation, not a conglobate bootstrap-stage pass. The Debian
seed provenance records its OCI origin and external importer identity.

The runner accepts `--reference` to name the pinned seed in the generated lock.
For recipes that package an existing firmware pair, `--firmware-pmi` and
`--firmware-dtb` copy both files into the committed context's `firmware/` directory.
They become measured context inputs; the binary does not fetch or select firmware.
The Debian recipe currently uses the known Fedora-built kernel/corium platform
pair with Debian userspace, then the default DTB transformation binds its new root.
That tests Debian APT/systemd and firmware-stage extraction, not Debian kernel
packaging or new arma firmware generation.


The disposable VM environment now loads a fixed exact-name SELinux-label
exception before invoking conglobate. `label_environment.py` builds the trusted
fixture loader outside the binary and retains its source in each run. Loading
happens only inside the VM. `--without-label-policy` is a harness negative-test
switch; conglobate has no policy-loading option or dependency.

The trusted guest init records the build script exit status on a separate 4-KiB
virtio result device and syncs it before reboot. Recipe commands cannot access
that device. The runner requires both the exact build/export console marker and
a successful result record, then verifies all exported blob hashes. This avoids
relying on an exit line that the kernel's reboot printk can interrupt; no sleep
is used. Expected failed builds still produce a failing runner verdict, with
negative-test assessments recorded separately.

`--network-fixture` stages the maintained lifecycle fixture as a trusted DNS/TCP
service in the disposable VM and waits for pipe-based readiness. It supplies
that service's address through the ordinary `--resolv-conf` input. The service
answers only the fixed `build.fixture` question and checks the expected TCP
request; it does not fetch artifacts. The runner waits for successful service
completion and checks that no conglobate veth links or nftables tables remain
on either build success or failure. The service has a finite observation deadline.

`network/recipe.yaml` exercises actual recipe DNS/TCP, read-only resolver injection,
and a following offline step. `network/nonzero.yaml` and `network/timeout.yaml`
reach the same exchange and then fail deliberately. Assess each completed run with
`python3 tests/distro/network/verify.py RUN success|nonzero|timeout`. Failed builds
still make the runner fail; the separate assessment requires the expected error,
independent rejection status, output absence and cleanup checks. These are local
VM-destination tests, not proof of external outbound NAT or IPv6 connectivity.

`--external-network-fixture` starts a separate ordinary-user service bound only to
host loopback on an ephemeral port. The port and per-run request token are added
to the committed context as `external-endpoint`. `network/external.yaml` copies
that input through COPY, reaches the endpoint via the VM's QEMU gateway and
requires its reply. The service independently records the exchange, is bounded
by a timeout, and is terminated/reaped on runner failure. No privileged host
network configuration occurs. Assess with `network/verify.py RUN external`.
The runner also starts trusted tcpdump inside the VM before admission, waits for
its readiness, and captures only the outgoing TCP SYN to the selected fixture
port. The safe Rust observer requires the source to be the VM's 10.0.2.15 and
the destination to be 10.0.2.2 at that port, before QEMU's outer translation.
Use `network/verify.py RUN external-nat` to require this observation as well as
external reply, cleanup and independent build status. `external` assesses only
connectivity for earlier runs. Tcpdump explicitly runs as root in the trusted
VM fixture because this minimal runtime has no tcpdump account. No host packet
capture occurs. IPv6 and unsolicited inbound filtering remain separate tests.

## Fixture-specific instructions

- [alpine](distro/alpine/README.md)
- [bootstrap-labeling](distro/bootstrap-labeling/README.md)
- [confinement](distro/confinement/README.md)
- [descendants](distro/descendants/README.md)
- [bootstrap-nonzero](distro/failures/bootstrap-nonzero/README.md)
- [firmware](distro/firmware/README.md)
- [label-operations](distro/label-operations/README.md)
- [labeling](distro/labeling/README.md)
- [labeling-fault](distro/labeling-fault/README.md)
- [nixos](distro/nixos/README.md)
- [Network resolver checks](distro/network/RESOLVER.md)
