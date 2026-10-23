# M4 adherence attempt at 168b2a2

Status: IN PROGRESS, no full pass awarded. This is the cumulative record for
this attempt; complete every remaining section before evaluating a clean pass.
The immediately preceding turn made implementation/test progress in 168b2a2.
No source change or new VM run occurred in this review segment.

## Requirements and authority

Latest user decisions override the original goal's policy-registration wording:
the environment supplies a fixed exact-security.selinux exception, and disposal
of the dedicated VM handles supervisor death. Conglobate does not register or
revoke policy. Normal success still requires complete process and filesystem
cleanup. Namespace/cgroup confinement and explicit fallible finalization remain
required. An environmental shutdown contract is not proof of deployed appliance
shutdown qualification.

## Segment A: cancellation, process ownership and failed preparation

Reviewed full source files listed in reviewed-source-sha256.json. Domain record
encoding/identity methods were also consulted, but the complete recovery/anchor
review is still pending and is not inferred from those excerpts.

1. Admission and cancellation are distinct capabilities. RecordedDomain launch
   retains domain ownership on launch/join failure. DomainJoin borrows the live
   domain, verifies its published record, and rechecks before exposing the join
   descriptor. ContainedHelper cancellation closes its channel before requesting
   cgroup cancellation. An admission acknowledgment failure can follow delivery;
   its failure owner permits cancellation, not a return to pre-admission state.
2. CommandDomain cancellation writes cgroup.kill, observes populated=0, then
   removes the owned directory. DomainWatch re-reads bounded cgroup.events after
   PRI notification; it does not use an always-ready IN poll or a sleep. Empty
   population is not treated as reaping or successful storage finalization.
3. HelperReaping collects the separately owned helper before DescendantDrain
   uses the process-wide ChildReaper. ChildReaper uses __WALL, maintains the
   exclusive wait prerequisite, checks its process/subreaper identity, and uses
   pidfd polling for live children. Its ECHILD receipt is only accepted after
   independent domain cancellation in the Step composition.
4. Process<Identified> signals through its pinned pidfd. Early unpaired helpers
   retain exclusive Child wait ownership; the documented normal-SIGCHLD and
   sole-waiter preconditions matter for numeric PID use. These are trusted
   supervisor process preconditions, not claims about arbitrary library callers.
   ChildLease Drop is bounded best effort; it produces no completion receipt.
5. Individual wait primitives can collect an already exited process, or observe
   an already empty domain, after a supplied cutoff. The Step/rollback/finalizer
   composition rechecks its fixed deadline at the next transition and before
   accepting completion. Therefore primitive success alone must not be cited as
   evidence of on-time lifecycle success. The current composed path retains its
   phase/storage on expiration.
6. Step states preserve namespace and storage through cancellation, reaping,
   draining and record cleanup. Only Finishing releases those owners. Mounted
   storage still requires finalization; StepCompletion is not ExecutedStorage.
   TeardownCompletion does not expose mounted storage through an into_parts API.
7. StoragePreparation failures retain the resources actually acquired: early
   helper wait owner, mount rollback, domain publication inspection, or assigned
   namespace/storage. Rollback follows consuming phase transitions under its
   original cutoff. Domain publication errors distinguish absent, exact and
   mismatched records; they never assume a failed publication wrote nothing.
   No recursive deletion of cgroup control files is used.
8. Finalization preserves the cutoff through unmount/retry, validates snapshot
   state, and checks time again before producing FinalizedStorage. Execution
   acceptance additionally requires successful primary outcome. Cleanup retry
   returns StorageExecutionFailure, with concrete error history retained; it
   cannot manufacture ExecutedStorage after a prior cleanup failure.
9. Diagnostic relay threads live in the outer trusted receiver. Pipe endpoints
   reaching command code provide diagnostic output only. Joining happens after
   namespace child exit and is bounded by the outer supervisor lifetime, rather
   than a second independent receiver timer. Detached relay fallback on local
   failure is not treated as completed cleanup; helper process exit/cancellation
   remains necessary.
10. Style: sealed state traits and owning transitions carry process/Step
    authority. Rollback phase enums model failure alternatives with concrete
    resources; they are not mutable acceptance flags. ManuallyDrop retention is
    deliberate because Drop cannot certify quiescence. FailureDiagnostics uses
    consuming history extension and read-only access. No new unsafe application
    code, free-standing orchestration function or sleep synchronization was
    found in this reviewed segment.

No new defect was established in this segment. This statement does not cover
unreviewed modules, unrestricted library consumers, or every acquisition race.

## Evidence fidelity checked in this segment

`evidence-source-comparison.json` reads preserved VM assessments and compares
all 18 reviewed source files against each executed source archive. No reviewed
file was missing from those archives. This is a subset comparison, not a claim
that an entire historical build matches current code.

- Older contained/publication/namespace/mount/launch/join/attached-exit and
  writeback runs match the reviewed domain, process, preparation and finalizer
  files. Differences in the selected set are the ObservedCommand re-export,
  rejection-only execution recovery, and Clone on diagnostic history. The
  historical execution retry API must not be represented as current coverage.
- status-busy-execution-v1 includes the rejection-only retry implementation;
  among this selected file set, only the ObservedCommand re-export differs.
- observation-blocked-descendant-v1 and delayed-teardown-v1 match all reviewed
  files. They directly exercise new completion/cutoff ownership, but neither is
  a replacement for all older preparation and successful cleanup cases.
- M4-T1 still requires the integrated admission/descriptor/gate and evidence
  scope review. The latest cutoff regression fixes the newly found API issue;
  this review does not silently award a full M4 pass from its passing tests.

## Remaining sections for this attempt

- Environment assumptions, command capabilities/seccomp, namespace creation,
  descriptor/proc boundaries, root confinement, runtime mount targets, admission
  and shell/cleanup protocols (including the complete M4-T1 reconciliation).
- Workspace/cache/verity/scute chain, loop/device-mapper ownership, filesystem
  mount/unmount/idmapping, pinned recovery anchors and serialized records.
- Network preparation/cleanup and resolver authority as coupled M4 requirements;
  retain explicit M6/M7 dual-stack and inbound-traffic qualification gaps.
- Full M4 evidence sufficiency, historical finding dispositions, all errors and
  API consumers across those sections; preservation and external-asset limits.

M5/M6 and platform/distro/CI/retirement gates remain separate obligations. When
all sections are reviewed, record the full result; findings reset the streak.
Do not count this segment, or a second segment, as separate clean passes.

## Segment B: command isolation and init status authority

Production scope is bound by isolation-source-sha256.json (14 files). Also read
confinement recipe/verifier behavior, maintained command-probe assertions and
preserved clone-filter/label-admission assessments. No production changes or new
execution in this segment. No new defect established within the reviewed scope.

1. UserNamespace creates a fresh namespace through a trusted, wait-owned helper,
   excludes parent root/invalid IDs, rejects preexisting maps, and writes each
   mapping once. The creator is reaped before returning the descriptor owner.
   Disjoint ID-range allocation remains the trusted caller's responsibility.
2. Namespace entry clears supplementary groups before joining mapped user root.
   Configured networking is joined before surrendering initial-user-namespace
   authority; offline networking gets a fresh namespace and loopback. Mount,
   IPC, UTS and child PID namespaces are private. Namespace descriptors close
   during entry; root setup is the next child and must be PID 1.
3. PrivateMountNamespace retains restoration authority only during trusted
   setup. Root entry commits that guard, closes its original-namespace handle,
   pivots using the pinned assigned root, detaches the old root, and changes cwd
   to /. Partial failure is exit-only in the production helper contract.
4. Runtime targets resolve with IN_ROOT/NO_MAGICLINKS/NO_XDEV. Alias and ancestor
   checks reject root, duplicate and nested targets before attaching runtime
   filesystems. This depends on the trusted exclusive pre-admission view; it is
   not a general concurrent filesystem service. Proc is new for the PID
   namespace, /sys is empty/read-only, and selected proc controls are read-only.
5. /dev is fresh tmpfs with only approved device nodes, owned by the mapped IDs.
   It does not bind host device inodes. Private devpts and /dev/shm are created
   under pinned trusted targets; setsid removes the inherited controlling tty.
   The image mount's nodev requirement is reviewed separately with filesystem
   attachment. No context, cache or device-mapper authority is added here.
6. Capability setup queries all supported ABI bits and verifies effective,
   permitted, bounding, inheritable and ambient results. It removes SYS_ADMIN,
   SYS_PTRACE and other capabilities outside the filesystem profile, sets
   no_new_privs and clears ambient/inheritable sets. Errors cannot continue to
   image exec. This allowlist is still subject to distro compatibility review.
7. Native seccomp defaults to EPERM, with a separate clone3 ENOSYS fallback.
   Namespace clone flags are masked out; unshare/setns/mount/bpf/ptrace/keyctl
   are not ordinary allowed operations. Socket families, prctl and ioctl are
   restricted; mknod/mknodat only admit FIFO mode. ABI guards and source support
   for aarch64 do not establish tested Arm compatibility.
8. Trusted setup messages require exact opcode/length/descriptor-count shapes,
   reject truncation, and receive rights CLOEXEC. nsfs/tmpfs object checks are
   sanity checks, not authentication for arbitrary senders: private trusted
   handoff and enclosing ownership provide authority. Malformed-message tests
   check received-right closure rather than racy process-wide FD counts.
9. Root setup replaces stdin with trusted null, clears environment, restricts
   capabilities, makes resident init non-dumpable and installs seccomp before
   spawning /bin/sh -eu -c. Descriptor audit permits only standard descriptors
   and the exact owned CLOEXEC completion endpoint. Init verifies mapped root
   and PID 1. It reports the reaped shell's status before namespace exit, then
   the outer lifecycle still requires cleanup/quiescence/finalization.
10. Gate typestate separates Waiting, Ready and Started. CompletionEndpoint is
    consumed by its one nonblocking status report. Exact terminal wait-status
    encoding is validated; malformed packets or ancillary descriptors fail.
    The endpoint exposes no raw handle to public consumers. Diagnostic pipes
    are separate from this protocol and are not decoded as status messages.
11. Label admission probes only anonymous inodes on assigned roots after
    privilege restrictions. It requires successful write/read/remove of the
    foreign security.selinux value before start. Failure aborts admission;
    the binary does not install or modify BPF policy. Exact-name environment
    policy semantics and both-root bootstrap qualification remain coupled
    review obligations, not proven by this anonymous probe alone.
12. Safe Rust methods and consuming transitions carry setup authority; thread-
    local guards cannot be sent across threads. Optional network/resolver and
    optional status endpoint describe supported inputs/paths, not mutable
    acceptance flags. Low-level ungated APIs have explicit trusted-caller
    preconditions; production uses the gated composition.

### Evidence scope and source comparison

`isolation-evidence-comparison.json` compares all selected files against five
preserved source snapshots, with no missing files. descendant-status-v1 and
observation-blocked-descendant-v1 match all 14 current files. The former's actual
CLI confinement recipe verifies five capability sets, mapped IDs, no live extra
shell descriptors, PID-1 proc access denials, private runtime recreation, selected
syscall denials with controls, and successful command/process/socket use.
The latter demonstrates early shell status with a real D-state descendant.
Neither constitutes comprehensive escape testing or platform qualification.

clone-filter-v2 matches the current syscall and capability policy, but predates
other isolation changes. Its controls establish seven legacy namespace-clone
flag denials and ordinary-clone success; it does not test NEWTIME, Arm, or the
whole current sandbox composition. Proc observations accept empty/absent key
and scheduler interfaces; they do not prove seeded-key isolation or denial of
a present sched_debug file on another kernel configuration.

label-admission-off/on-v2 assessments demonstrate historical no-policy rejection
and enabled-policy build success. Their snapshots differ from the current
namespace/gate implementation. The label probe changes add contextual errors
and invalid-tag tests; namespace pin retirement and later status handling are
substantive composition changes. Do not label these entire runs current-source
coverage. Current successful admission is exercised by the matching confinement
and blocked-descendant runs. Current negative-policy composition and both-root
fault evidence still need reconciliation with the newer label-unpinned and
bootstrap evidence in the remaining integrated review.

### Remaining scope after segments A and B

- Complete supervisor identity/join/volume protocols and receiver cleanup
  composition, including the final M4-T1 evidence reconciliation.
- Environment-policy exact-name controls, applicable newer negative admission
  and bootstrap fault evidence; architecture/platform limitations.
- Workspace/cache/verity/scute chain, mapping/loop ownership, filesystem views
  and idmapping, recovery anchors and record serialization.
- Network/resolver preparation and teardown, historical finding dispositions,
  full evidence sufficiency and preservation status.

The earlier remaining-section list is refined by this list. No segment is a
full clean pass; M4 stays 0/3 until the complete attempt is evaluated.

## Segment C: storage backing, filesystem views and recovery anchors

Scope: 18 files in storage-source-sha256.json. Reviewed at unchanged production
baseline 168b2a2. No new defect established in this subset; no source change or
new test execution. Cache manifest resolution, sealing and complete pipeline
consumers remain separate review scope rather than assumed from these owners.

1. Workspace creates one detached root-owned 0700 tmpfs with nodev/nosuid/noexec.
   Scratch is anonymous and borrowed from it; no writable raw descriptor or
   clone escapes publicly. Snapshot capacity is sparse logical capacity, not a
   claim that corresponding RAM is available. Allocation/write errors do not
   fall back to disk. Ordinary tmpfs/swap policy remains environment-owned.
2. VerifiedChain authenticates compressed/plain blob reads and derives ancestry
   from computed roots and parent salts. Duplicate/disconnected roots fail;
   removing candidates bounds ancestry traversal even for cycles. PreparedScute
   retains the exact open inode used to construct the verity tree. Aligned plain
   scutes stay cache-backed; compression or a partial final block materializes
   in RAM. This does not promise every immutable format stays cache-backed.
3. Preparation authenticates the current read, not future cache contents.
   VerityMapping keeps read-only data/hash loops and the prepared owner; kernel
   reads are checked against the trusted root. Snapshot headers are read through
   that mapping before snapshot-target activation, with fixed supported chunk
   size and valid-state checks. Paths cannot redirect a pinned inode afterward.
4. ReadOnlyChain assembles base-to-top over the fixed dm-zero origin and closes
   top-to-base. Failure variants retain exactly the active mapping, verity,
   backing and lower stack still requiring cleanup. Produced-layer parent roots
   must match the current chain; creating a root requires the zero parent.
   A copied device ID is documented diagnostic data, not a lifetime lease.
5. Writable SnapshotMapping borrows its origin and owns its loop backing. Those
   owners remain on activation/removal failure. Mapping activation is consuming
   Created-to-Active; names are newly generated and collisions are errors, never
   authority to adopt/remove an existing mapping. Active mappings cannot reload
   through the public API. Snapshot health rejects invalid/overflow/unknown and
   inconsistent counters; health observation alone is not sealing permission.
6. Loop clear is not assumed to mean immediate detachment. Explicit writable
   detach flushes and requires unbound status before returning Scratch. Read-only
   detach similarly checks actual state. Deferred DM removal and loop autoclear
   in destructor paths keep kernel backing references, but never return an
   explicit successful cleanup receipt.
7. Volume couples mounted filesystem resources to snapshot ownership. Views
   borrow the volume and clone nonrecursively. The ext4 mount uses fixed
   nodev/nosuid flags and a verified owned device descriptor; no recipe mount
   options reach this operation. COPY requires exclusive borrowing. Low-level
   mounting requires the documented trusted parent; the production Runtime
   supplies protected directories. Direct library callers do not gain arbitrary
   parent authentication from the tmpfs type check alone.
8. Normal unmount verifies namespace, calls syncfs, closes the root pin, performs
   non-lazy unmount, then removes its empty attachment point. Each failure keeps
   the correct flush/mount/target owner. Lazy destructor detach is explicitly
   non-finalizing. Single-volume syscall duration is not interruptibly bounded;
   outer lifecycle checks before/after it reject late completion. This is the
   accepted kernel-blocked-operation limitation, not a guaranteed kill time.
9. Idmapped detached views keep namespace identity and set nosuid/nodev. The
   scope borrow prevents normal restoration while they are live. Bootstrap pairs
   require the same user namespace and confined ordinary /mnt target; attachment
   failure owns both partial views. InUse views retain child wait ownership and
   cannot be explicitly closed before that child is reaped. Direct child exit
   still is not proof of all descendant quiescence.
10. BootstrapVolumes keeps tool/output together through partial mount/unmount
    states. Output unmount precedes tool unmount; a partial pair cannot escape
    through the failure API. Step preparation/finalization uses the original
    cutoff. The public low-level unbounded unmount helper is not substituted for
    that bounded acceptance composition. Actual formatting and output sealing
    remain M5 review obligations.
11. Runtime attaches control backing from the same workspace and mounts cgroup2
    under a protected fresh path. Namespace identity checks bind it to a live,
    independent deployment init. Close requires empty recovery/volume/domain
    directories before unmounting control resources; no recursive deletion or
    lazy detach turns live command resources into successful runtime closure.
    Acquisition failures retain the attachment list and their cleanup cutoff.
12. RecoveryDirectory verifies attached tmpfs, root ownership, exact 0700 mode,
    init PID/mount namespace and a nonblocking exclusive cooperative lease.
    Derived owners share lock lifetime through Arc without raw-FD/early-unlock
    access. This excludes cooperating owners, not an independently privileged
    environment replacing paths; that trust limitation is explicit.
13. ControlRecords bounds names/content, uses nofollow/nonblocking reads with
    owner/mode/type checks, and publishes without replacement. Publication may
    succeed before a later sync error, so domain rollback inspects actual records.
    Record removal checks expected identity and target absence; it does not
    independently certify process reaping or allow output publication.
14. Style: immutable verified inputs, sealed mapping/volume states, lifetime
    borrows and consuming failure owners carry authority. Option fields in
    destructor leases track whether ownership has moved, not whether an image
    is accepted. Runtime's private attachment vector records incremental kernel
    acquisitions for rollback; it is not externally mutable build state. No
    unsafe application code or new free-standing orchestration was found here.

### Storage evidence fidelity

storage-evidence-comparison.json compares all 18 files to six executed source
archives: cache-storage-v2, chain-storage-v1, maintained-runtime-v1, writeback-v3,
maintained-domain-publication-v1 and observation-blocked-descendant-v1. Every
selected file is present and byte-identical. Assessments are preserved in the
comparison; their scope still matters:

- Cache tests cover aligned backing, materialization cases, read-time corruption
  rejection and retained diagnostics. Chain tests establish two-layer ordering,
  zero fallback, read-only parents, writable top snapshot and invalid-header
  rollback. They are synthetic block tests, not a distro boot.
- Writeback tests force owned snapshot exhaustion and require rejected
  finalization/retry. Publication and runtime tests cover selected acquisition
  and cleanup failure paths, not every possible kernel failure.
- Latest blocked-descendant test retains storage on cleanup expiration; it
  expressly does not prove successful release of an uninterruptible task.
- Exact source matches in this selected set do not imply all callers or the
  whole historical binary match current production. Earlier sections record
  relevant execution/cutoff changes separately.

### Remaining scope after segment C

Continue supervisor identity/join/volume protocols, receiver cleanup and M4-T1
reconciliation; network/resolver ownership; current environment-label controls;
sealing and pipeline consumers at the M4/M5 boundary; historical finding
reconciliation and complete evidence/preservation assessment. This cumulative
attempt still has no full clean pass and M4 remains 0/3.
