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


v17 replaces fixture numeric-PID placement with production mutual supervision
and registration. A dedicated helper receives cgroup.procs over the private
channel, writes its own membership, closes the control FD and acknowledges before
creating its sleep child. The probe checks the descriptor is gone, observes two
domain members and cancels/reaps as before. v18 adds a helper that exits after
identity exchange without registration acknowledgement: no Registered state is
returned, the child is reaped and the still-owned empty domain is cancelled.
These validate registration mechanics; persisted domain ownership, final sandbox
admission, guardian failover and concurrent-death recovery remain unimplemented.


v19 requires the production recovery lease for command domains. The probe drops
the facade while an empty domain remains live; both a local competing open and a
separate probe process are excluded. Successful cancellation releases the lock.
After a forced removal failure, the returned domain also retains exclusion; it
still supports registration and cancellation without the facade. Final cleanup
allows an independent process to acquire the lease. v20 additionally attempts
cancellation of an otherwise removable empty domain from a private mount
namespace: it is rejected and returns the owner, which works after restoration.
These qualify lock lifetime and namespace checks, not persisted domain recovery.


v21 saves a canonical production domain record before registration, rejects
replacement publication and premature record deletion, and rejects a wrong
parent identity. With the helper and descendant still alive, it drops the domain
owner, reacquires the lease, loads/reopens the recorded domain and cancels it.
Record removal succeeds only after the domain directory is gone. Existing
namespace record file checks also exercise the shared storage implementation.

v22 additionally rejects saving through another recovery lease without writing
anything there. It deletes an owned empty cgroup, recreates a private directory
with the same name and verifies recovery rejects its different identity. A
separate publisher saves an empty domain and exits without Rust destructors;
the parent recovers/cancels it and removes the record. Both runs pass. These are
component persistence checks, not complete supervisor/guardian crash recovery.


v23 makes the protected record a production registration prerequisite. Unsaved
domains cannot obtain DomainRegistration. The fixture obtains a capability,
corrupts its owned record, then attempts registration: the recheck rejects it,
the channel closes, and the helper exits without joining or creating a child.
Restoring the canonical record permits the normal registration/cancellation path.
The missing-ACK fixture now persists its domain before attempting registration,
then cleans its record after cancellation. Earlier persistence/namespace/policy
checks remain. This gate covers the domain record, not the full build lease.


v24 adds production GuardianWatch after helper registration. A separate trusted
process stands in for the supervisor peer outside the command domain. The watch
times out with both peers alive; a fixture thread then terminates that peer while
the main thread waits, and SupervisorLost is observed. The fixture joins/reaps
the terminator's child owner, reopens/cancels the populated domain, reaps the
registered helper and observes BothLost. Prior checks remain. This tests the
registered-phase observation/cleanup sequence, not a complete guardian process,
setup-phase monitoring or concurrent supervisor/guardian death recovery.


v25 introduces the recorded-domain aggregate fixture: publication, failed domain
removal with ownership recovery, registration, owner-drop/reopening of a populated
domain, cancellation, then record-cleanup failure and retry after restoring the
fixture-corrupted record. It passes, but review found its returned cancellation
owner still allowed registration. v26 tightens that boundary: cancellation failure
and recovery return a cleanup-only DomainCancellation. The fixture completes its
first failed cancellation, creates a fresh domain for registration, then recovers
that domain solely for cancellation. It passes the final ownership contract.

The initial aggregate probe compile caught borrowed cleanup errors escaping into
a static boxed-error return. Expected failure paths retain/retry their owners;
unexpected terminal fixture errors are rendered as diagnostics before the disposable
guest exits. This is not a production error-erasure policy. v25 preserves its
owner/probe source snapshots; v26 additionally captures the final module exports.
Neither run establishes complete guardian or policy/filesystem teardown.


v27 uses production ContainedHelper to combine domain publication, helper identity
exchange and registration. A retained empty child cgroup forces cancellation
removal failure; retry retains the process and domain, then reaps the helper.
Corrupting the fixture-owned record forces cleanup failure, with successful retry
after canonical restoration. Separate cases cover a helper closing before its
registration ACK, a nonexistent executable and a started sleep process timing out
during identity exchange. Each failure retains and explicitly cleans its resources.
The final lease can be acquired independently and the directory is empty. The
helper's exit status is diagnostic: channel closure may let it exit normally
before cgroup.kill arrives. The run passes with the dedicated lifecycle marker.
This does not exercise a forced reap failure, all-descendant reaping, full guardian
startup/crash arbitration or policy/mount teardown. Source and hashes are preserved.


v28 integrates the production RegisteredGuardian owner with a recorded contained
helper and a separate supervisor-peer process. One case times out with both peers
alive, the other terminates the supervisor peer. Both consume guardian ownership,
cancel the domain, reap the helper and remove its record. v29 adds a retained child
cgroup: supervisor loss still requests cancellation, failed removal retains a
cleanup-only owner, and removing the fixture obstruction permits retry and reaping.
Both runs retain sources and binary/kernel/initramfs hashes. This is registered-
phase orchestration in the trusted probe process, not a complete independently
launched guardian or full policy/mount/crash-recovery qualification.


v30 runs ChildReaper in a separate disposable-guest process before child creation.
A shell starts a benign background sleep and exits; its explicitly reaped status
leaves the sleep adopted by the production subreaper. An immediate deadline times
out without losing the owner, then retry collects exactly one orphan and confirms
an empty children list. Duplicate installation is rejected. v31 also rejects
installation with an existing directly owned child; explicitly terminating/reaping
it permits installation. Process-wide changes occur only in that guest child.
__WALL is used, but this fixture does not separately exercise non-SIGCHLD clone
children. No complete guardian/policy teardown proof is claimed by these tests.


v32 connects production helper cancellation/reaping to DescendantDrain in a
separate guest process. Its registered helper creates a benign sleep and exits,
leaving the orphan in its cgroup. Cancellation removes the domain; collecting
the helper status precedes draining the adopted child. An expired drain deadline
forces ownership-preserving failure, followed by retry; the protected record is
retained throughout. Record corruption then forces cleanup failure and successful
retry after restoration. The final count is one adopted child, and no children or
recovery files remain. This checks the integrated cleanup path, not independent
guardian launch, setup supervision or policy/mount teardown.


v33 runs the production lifecycle primitives in independent guardian processes.
One supervisor closes its channel while alive; another exits without Rust
destructors. The guardian retains the recovery lease and performs domain
cancellation, helper reaping, descendant draining and record cleanup. The fixture
checks lease exclusion before cancellation and availability after cleanup. An
outer subreaper collects the guardian orphaned by the departing supervisor.

v34 additionally terminates the guardian through its identified Process owner.
The surviving supervisor detects loss, acquires the abandoned lease, recovers
and cancels the recorded domain, drains adopted children and removes the record.
The outer subreaper's expected count accounts for whether the helper reaped its
sleep before cancellation. Readiness publication uses rename to avoid observing
an incomplete name. These fixture files synchronize tests only; they do not
implement production readiness/admission. Neither run establishes policy/mount
persistence, setup monitoring or simultaneous supervisor/guardian death recovery.
