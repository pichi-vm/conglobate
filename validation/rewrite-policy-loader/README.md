# Safe Rust disabled-policy loader validation

The probe calls the production LabelPolicy<Prepared> -> LoadedDisabled API.
It loads no external object: build.rs compiles and embeds policy/labels.bpf.c.
The library opens the embedded object, verifies map layouts and program sections,
loads it through libbpf-rs, checks zero config and populates targets. The current
probe then exercises disabled hook attachment/pinning and cleanup. Config stays
zero throughout; this cannot enable the exception.

Build and run (new output directory required):

```
TMPDIR=/var/tmp/conglobate-rewrite-tmp cargo build --locked --manifest-path validation/rewrite-policy-loader/Cargo.toml
TMPDIR=/var/tmp/conglobate-rewrite-tmp python3 validation/rewrite-policy-loader/run.py /var/tmp/NEW-loader
```

The runner derives from the preserved mount-api VM fixture. It uses the same
local seed, kernel and tooling prerequisites, copies the compiled probe and its
shared libraries, then runs it in a no-NIC/no-disk/no-share QEMU guest. Kernel
operations, capability changes and temporary mounts occur only in that guest.
The marker check prevents accidental invocation outside the fixture; it is not
an authorization mechanism.

Evidence v1 covers three production load/drop cycles with the process descriptor
count returning to baseline. Evidence v2 adds clearing only the probe thread's
effective capabilities, a failed load with no descriptor leak, capability
restoration and successful reload/drop. Both runs pass. v2 uses the then-current
formatted probe source and includes current source hashes; v1 preserves the
initial binary/input evidence without claiming final source identity.

v3 adds disabled pinning and observes one extra FD after cleanup. The witness
held a Vec<DirEntry>, which itself retains a read_dir descriptor on Linux. This
failed run and its probe source are preserved. v4 drops that witness before
counting; production code is unchanged between those runs. v4 passes explicit
close and Drop with all five expected pins present initially and an empty parent
afterward. It injects a directory collision at check_remove: config, targets and
check_set pins exist when installation fails. Cleanup also fails on that
collision, retains recovery ownership, then succeeds after the fixture removes
the collision. Descriptor counts return to baseline.

v5 adds production namespace pinning on protected tmpfs. The probe drops the
userspace pin owner, terminates the namespace creator through its pidfd, observes
exit, reopens the recorded pin, and tests denied-unmount ownership recovery before
successful release. The fixture shell reaps the creator after the probe returns.
v6 additionally tests mount denial retaining the unmounted target for cleanup and
rejects a substituted symlink even when it names the recorded namespace. Both
runs pass. The probe operates in the fixture init's mount namespace; these checks
do not implement or validate production selection of an independent anchor
namespace. They also do not exercise death of a production supervisor/guardian.

Review then tightened namespace-pin parent permissions: nsfs mount permissions
can expose handles even when the original target file was private. v7 rejects a
non-private parent and reruns the lifecycle with mode 0700. It passes and is the
qualification run for that final permission rule; v5/v6 retain earlier evidence.

v8 requires the production RecoveryDirectory lease for pin creation/recovery.
A competing open returns WouldBlock; a live pin retains the lock after dropping
the facade. v9 also checks the failed-creation target retains it, and moves the
directory before recovering through the original descriptor-backed pin. v10
executes an independent probe process: acquisition fails while held and succeeds
after release. All three runs pass. Locking is cooperative and does not establish
the lifetime of the containing mount namespace or persist the identity records.

Descriptor accounting checks process-held resources, not a global kernel leak
proof. These tests do not enable policy or qualify durable anchors,
command containment, supervisor/guardian death,
concurrent builds, Arm or confidential hardware. Existing C-loader policy
semantics evidence remains in ../rewrite-label-policy; that is separate from
this production Rust loading test.


v11 persists production namespace records as canonical CBOR under the directory
lease. It rejects duplicate publication and truncated data, refuses record
removal while the pin exists, and recovers using the stored record after the
creator exits. It removes the record only after successful unmount/unlink. v12
adds oversized-file, symlink, public-mode and mismatched-name rejection. Both
retain the earlier mount, locking, policy and descriptor-accounting checks.
The v11 probe source snapshot was reconstructed immediately from the v12 edit
by reversing only the added negative cases; its binary and input hashes are
captured directly from the executed VM artifacts. No production activation,
independent anchor namespace, complete build lease or crash protocol is claimed.


v13 adds the production deployment-init namespace check. It fails at the older
mount-denial fixture: clearing every effective capability prevents the new procfs
namespace check, so creation correctly fails before target allocation. v14 removes
only SYS_ADMIN in mount/unmount-denial tests, retaining the capability needed to
inspect init. The BPF load-denial test still clears all effective capabilities.
Production guard code is unchanged between these two runs.

v14 also carries a live lease into a private mount namespace: new lease acquisition,
record reads, creation and release are rejected; release returns the original pin
owner, which works after restoring the namespace. A separate publisher then saves
a pin/record and exits with std::process::exit (bypassing Rust destructors). The
parent reacquires the lease, reads the record and recovers/releases the pin. This
proves process-exit persistence in the guest init's namespace, not complete guardian
or concurrent supervisor/guardian death recovery. The v13 probe source was retained
by reversing the immediately following publisher-test edit; executed binary hashes
come directly from the VM artifact directory.


v15 adds production CommandDomain creation/cancellation on a guest-only cgroup2
mount. An empty domain is cancelled/removed. A trusted fixture shell blocks on
stdin before it can fork; the fixture moves it into the fresh domain, releases
that gate, observes shell plus child membership, and calls production subtree
cancellation. The shell is separately reaped and the domain disappears. v16
first creates an empty nested cgroup that prevents parent removal: cancellation
returns its domain owner, removal of the fixture child permits subsequent reuse
and successful cancellation. Both runs retain the namespace/policy checks.
This uses fixture registration, not a production registration/admission protocol.
No controller/resource limits or host cgroup operations are performed.
