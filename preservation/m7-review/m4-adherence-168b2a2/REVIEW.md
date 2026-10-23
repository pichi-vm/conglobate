# M4 adherence attempt at 168b2a2

Status: NEEDS WORK, no full pass awarded. Segments A–G establish the reviewed
source/evidence scope. The disposition below records unresolved requirements;
this attempt cannot serve as one of the three consecutive clean passes.
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


## Segment D: supervisor/helper protocol and command observation

Reviewed the 11 files in protocol-source-sha256.json, continuing the same
production baseline. No new functional defect established in this subset.
This is not a full clean pass.

1. Identity uses a private sequence-packet socketpair and a self-opened pidfd,
   avoiding parent numeric-PID lookup. Exact version/opcode, payload length,
   descriptor count and ancillary truncation checks fail closed; received rights
   are CLOEXEC. Socket possession and trusted startup are the authority: the
   decoder does not prove an arbitrary supplied FD is a self-pidfd. It must not
   become an untrusted public endpoint. Observation never substitutes for reap.
2. ControlSocket shutdown explicitly cancels despite transient duplicate FDs.
   pidfd exit detects abnormal peer death independently of socket closure.
   Unexpected traffic remains queued when sampling peer health, so a repeated
   sample cannot consume a malformed message into an apparently healthy state.
3. Sealed Unpaired/Paired/Joined/Admitted states express protocol progression.
   Joining requires the published DomainJoin capability on the supervisor side;
   the trusted single-threaded helper writes its own membership before closing
   the cgroup FD and acknowledging. The cgroup filesystem check is not independent
   authentication of an arbitrary sender. Prepared and admission are separate.
4. Volume/bootstrap/network packets have exact descriptor order and count.
   Raw resolver cleanup view is checked against root device/inode before use.
   Namespace/root metadata checks are structural; trusted transfer and later
   kernel operations supply the remaining preconditions. No received control FD
   is intended to survive into image commands. The supervisor retains backing.
5. Helper launch is bounded canonical versioned CBOR in argv, with environment
   cleared and no fault-selection role. Parsing alone grants no privilege. All
   execution paths exit the dedicated helper; partial namespace entry cannot
   return into the supervisor. The trusted executable runs before root entry,
   with no binary copy into the image. Formatter and network dispatch are
   identified here; their implementations remain separate review scopes.
6. Receiver waits for restricted readiness, explicit supervisor admission, then
   opens the execution gate. Setup failures preserve the live mount/child state
   under the original setup cutoff. Error rendering before helper exit does not
   confer cleanup success; parent independently retains cancellation resources.
7. Completion is a terminal shell wait status, validated separately from cleanup.
   Receiver relays it before waiting for namespace helper exit/diagnostic drain.
   The helper's normalized exit must agree with reported shell status. Resolver
   release, view close and namespace restoration precede Cleaned. The receiver's
   local send/wait budgets cannot extend the supervisor's cleanup acceptance
   deadline; its blocking work remains externally bounded by cancellation.
8. RunningCommand observation consumes observation authority and records the
   fixed cleanup cutoff. Exited outcomes wait for Cleaned; observation failure
   cancels immediately. Missing/wrong cleanup fails with the running owner
   retained; retry can cancel under the same cutoff but cannot recover an
   accepted execution. Cleaned alone is not sufficient: cancellation, helper
   reap, adopted-child drain, domain record removal and storage finalization
   remain required. Every transition checks the original cleanup deadline.
9. The private Teardown enum owns distinct typed phase resources, rather than
   exposing mutable state flags. Immutable launch values, consuming transitions,
   private failure owners and RAII carry authority. Serialization methods and
   transport methods remain associated with meaningful objects; no unsafe code
   was introduced. These are trusted orchestration interfaces, not an assertion
   that any arbitrary library caller can safely impersonate a helper.

### Protocol evidence fidelity and M4-T1

protocol-evidence-comparison.json compares all 11 source files against the
preserved observation-blocked-descendant-v1, observation-held-cleanup-v1 and
delayed-teardown-v1 archives. All are present and identical; each saved
assessment passed. The first exercises the production receiver/init with an
actually blocked descendant and requires retained storage on cleanup expiry.
The latter two are controlled-helper tests of separate cleanup timing and the
nonrenewable observation cutoff. They do not substitute for production resolver
coverage or demonstrate successful removal of a kernel-blocked task.

Together with the prior status gate review, these source transitions explain
why the original M4-T1 timing defect is addressed: recipe observation no longer
waits for descendant/receiver cleanup, and early status cannot release storage.
The accepted kernel-blocking limitation and rejection-only cleanup failures
remain explicit. Network-specific resolver cleanup evidence is still to be
reconciled in the network segment. No new execution occurred during this segment.

Remaining full-review scope: network/resolver ownership and evidence; current
external label-policy controls and dual-root failures; pipeline/sealing consumers;
historical finding reconciliation and complete preservation assessment. M4 0/3.


## Segment E: network ownership, NAT and resolver injection

Reviewed all production network modules plus Step's network storage adapter
(16 files in network-source-sha256.json). This continues the same full review;
it is not another adherence pass. No new functional defect established here.
The previously recorded reachability gap and missing qualification remain open.

1. CommandNetworking captures an immutable resolver input and workspace borrow.
   Enabled setup checks both environment forwarding settings without changing
   them. A nonblocking lease on the network namespace serializes cooperating
   allocators through topology capture, exclusive link creation and configuration.
   This does not exclude unrelated privileged environment network changes.
2. Topology examines assigned addresses and nondefault routes in both families,
   across route dumps, and chooses /30 and /126 links plus unused names. Interrupted
   dumps fail. It cannot discover destinations behind default routes, so the
   known collision/reachability issue in link-local-nat.md remains unresolved.
   Link-local was not adopted or validated by this review. Visible topology
   avoidance is not proof that all previously reachable destinations remain so.
3. Namespace creator unshares before identity exchange; the parent pins its net
   namespace and verifies separation/user namespace before reaping the creator.
   No readiness sleeps. Peer configuration runs in a fresh trusted helper and
   retains the worker/connection on failure. Commands enter the configured net
   namespace before dropping initial user-namespace authority.
4. Veth acquisition is exclusive, initially down, with generated MAC ownership
   tags and protected control markers. Operations verify the pinned supervisor
   net namespace. Failed creation never authorizes deleting a foreign same-name
   link. Tags are cooperative ownership checks, not protection against another
   privileged actor deliberately replacing interfaces. Abandonment retains the
   marker; successful explicit deletion removes it.
5. Netlink sockets are nonblocking/CLOEXEC, kernel unicast replies are checked,
   truncated/framing/sequence/duplicate/interrupted replies fail. Route requests
   each use a fresh socket; nft transactions track individual acknowledgements
   and ignore older transactions' ACKs without extending their deadline.
   Lost acknowledgement is not assumed to mean no mutation occurred.
6. NAT's nft table is socket-owned, exclusively created, with transactional rules.
   Forward/output traffic toward the command link admits established/related
   replies and otherwise drops, except local IPv6 neighbor discovery. Masquerade
   matches traffic arriving from the command interface. No destination filter
   was added. Existing environment policy can still deny traffic; this table's
   accept verdict does not override all other tables. VM-routable services are
   not isolated merely by using a separate command namespace.
7. Explicit network close requires acknowledged policy removal before interface
   deletion, then resolver source removal. Failure carries exact remaining
   owners and the original cutoff; retries do not renew it. Setup failures never
   yield an admissible network. Step stores network and volumes together until
   descendant teardown, then closes networking before storage finalization.
   Low-level public close methods require trusted callers to establish quiescence.
8. ResolverInput pins a regular file before reopening, captures bytes once,
   rejects observed length changes, hides contents in Debug and does not invent
   DNS servers. Concurrent same-length source writes are not an atomic snapshot
   guarantee; the supplied file is trusted environment input. No source path is
   reopened between steps. Inventory exclusion follows the explicit user decision.
9. Source bytes live on workspace tmpfs with no retained writable FD. Detached
   views require read-only/nosuid/nodev/noexec tmpfs regular-file mounts. Target
   traversal follows ordinary symlinks within root, bounds expansion, rejects
   escapes/runtime targets/incompatible types, and kernel-rechecks the original
   spelling against the selected inode. Selection occurs before image writers.
10. Temporary scaffolding records pinned parents/inodes before further operations.
    Cleanup checks identity and removes only owned entries in reverse order;
    replacements/nonempty directories/unpinned mkdir obligations reject cleanup.
    Existing files/symlinks are preserved. Receiver restores scaffolding after
    namespace command exit and before Cleaned. A failure discards the build layer;
    destructor behavior does not turn it into an accepted output.
11. Style uses immutable address/configuration values, consuming connection/policy
    transitions and owned error phases. The internal cleanup enum carries actual
    resources rather than public mutable flags. Resolver's private vector tracks
    acquisitions for reverse cleanup. Netlink sequence mutation is transport
    state. No unsafe application code or free-standing orchestration introduced.

### Network evidence fidelity and outstanding coverage

network-evidence-comparison.json compares all 16 files to five preserved archives:
maintained-network-composite-v1, network-external-v3, status-network-resolver-v1,
status-network-nonzero-v1 and status-network-timeout-v1. All selected files match.
The comparison uses exact archive paths, avoiding the separate lifecycle fixture
also named src/network.rs. Saved assessments pass; no new executions were run.

- Composite fixture checks namespace flock exclusion/release, contention before
  mutation, partial setup cleanup, foreign nft-table preservation, distinct live
  allocations, and expired teardown/retry retaining resources. Its final retained
  state is disposed by the VM, not successful teardown.
- External-v3 observes actual outbound TCP outside the VM and the translated
  IPv4 SYN source before QEMU NAT. It does not establish IPv6 or unsolicited
  inbound denial. Its full production caller predates later status/cutoff changes.
- Later status runs cover actual recipe DNS/TCP with read-only resolver injection,
  nonzero/timeout rejection and no output, and a successful subsequent offline
  step verifying the original absolute resolver symlink and original bytes.
  These service endpoints are VM-local, not new external NAT evidence.
- Reviewed the maintained network verifier's independent status/output/cleanup
  checks and success recipe. Rootless resolver tests cover dangling paths and
  replacement cleanup mechanics, but do not qualify command-time concurrent
  rename/replacement of a mounted resolver in the complete sandbox.

Before declaring the network requirement qualified: settle default-route collision
handling (including any link-local experiment), validate unsolicited inbound denial
and IPv6, and qualify dangling/concurrently changed resolver targets in actual
commands. These are not waived by exact source comparisons or successful IPv4.
Remaining cumulative review: external label-policy controls/dual-root failures,
pipeline and sealing consumers, historical findings and preservation reconciliation.
M4 remains 0/3 full clean passes.


## Segment F: external label environment and admission evidence

Reviewed current label probe, its placement in the already-reviewed restricted
helper, maintained environment staging, fixed exception, differential fault
fixture/loader/verifier, missing-policy verifier and label-operation recipe.
Hashes and selected historical comparisons are in label-source-sha256.json and
label-evidence-comparison.json. No new functional defect established here; no
full clean pass or current complete-system negative-control qualification claimed.

1. The application contains the behavioral probe, not policy loading or subject/
   filesystem registration. Fixed policy is an environment obligation, matching
   the latest user decision. The exception compares the complete terminated
   security.selinux attribute name and preserves an earlier nonzero hook result.
   It does not authorize security.selinux.extra or grant command SYS_ADMIN.
2. Environment loader attaches and pins once, then exits. There are no subject
   maps in the normal policy and no runtime updates by conglobate. Fixture marker
   files guard against accidental execution outside the VM; they are not a
   security boundary. Host staging compiles/packages; guest init loads. Tests use
   selinux=0. This does not establish behavior with an enforcing VM policy or a
   kernel lacking the hook/BTF/BPF LSM support.
3. AssignedRoots derives from trusted volume handoff, not whether an image happens
   to contain /mnt. Restricted root setup applies credentials/capability/seccomp
   constraints before probing, and probes before readiness/admission. Bootstrap
   probes both / and the separately assigned /mnt. Open output rejects symlink
   substitution. Anonymous TMPFILE|EXCL inodes cannot become named probe artifacts;
   owned FDs close on success/error before the final image-command FD audit.
4. Each probe requires label set, exact readback and removal. Failure is admission
   rejection with contextual root diagnostics, not a fallback grant. A positive
   probe shows these operations under tested credentials, not a comprehensive
   audit of the supplied policy or an independent test that SELinux is disabled.
   The environment remains trusted and must meet that deployment precondition.
5. Differential fault policy is test-only: preserves prior errors, recognizes the
   exact production probe name/value, allows the first superblock and denies a
   different one. Its counter map is observed after the build, with no conglobate
   test hook. This map must not be confused with forbidden production per-command
   registration. Verifier requires observed device, exactly one allowed/denied
   write, independent failure status, absence of recipe sentinel/output and
   complete cleanup. Root-only control requires success and no denial.
6. Label-operation recipe writes foreign labels in both roots, checks rejection
   of adjacent/security.other/trusted attributes and a read-only /sys label write,
   then checks labels across a fresh command mount and removes/recreates them.
   The read-only /sys case is not proof for every cache-backed view or every xattr
   namespace. Remount persistence is not exported target-OS enforcing boot proof.
7. Safe Rust probe ownership and immutable AssignedRoots selection conform to the
   style preferences. The C policy/loader remains external validation input,
   outside application Rust. Maintained harness still sources the normal fixture
   from validation/global-label-policy and headers under /var/tmp: migration and
   durable dependency preservation are outstanding M8 obligations, not portable
   production packaging already delivered.

### Historical versus current label coverage

- label-unpinned-off-v1 shows missing-policy rejection and cleanup without policy
  pin inspection. Its probe differs by later diagnostic wrapping/tests; its
  receiver attachment and status-gate code also predate current production.
- label-operations-v1 establishes the two-root write/remount/removal behavior for
  its archived code. It likewise predates later receiver/gate changes.
- second-root-control-v1 and second-root-negative-v1 preserve the differential
  allowed=1/denied=0 and allowed=1/denied=1 results. Probe changes since those runs
  add contextual diagnostics, but integrated completion handling also changed.
- receiver-root-negative-v1 matches the current labels.rs and fault sources;
  helper/receiver/gate still predate the M4-T1 status/cleanup separation. This is
  strong admission-path precedent, not an exact-current whole-binary run.
- Current blocked-descendant/confinement runs exercise the fixed exception's
  positive path with newer completion code, as recorded in earlier segments.
  They do not replace a current missing-policy/differential failure run.

The comparison explicitly lists missing historical files rather than claiming
byte identity. A refreshed current negative-control run remains outstanding,
as do target enforcing boots and non-amd64/confidential-platform qualification.
No new VM run, policy load, source modification or asset deletion in this segment.
Remaining cumulative review: pipeline/sealing consumers and historical findings/
preservation reconciliation. Existing network qualification gaps remain open.
M4 remains 0/3.


## Segment G: pipeline consumers and sealing boundary

Reviewed the eight files in pipeline-source-sha256.json, tracing M4 lifecycle
receipts into the stage coordinators, firmware capture and snapshot sealing.
This is the lifecycle integration boundary, not a substitute for full M5/M6
formatter, COPY, transform, inventory or OCI publication reviews.

1. CompletedDirectives is constructed only after every directive succeeds.
   RUN consumes ExecutedStorage after command success and finalized storage;
   each subsequent RUN gets fresh mounts. COPY executes between quiescent commands,
   requires mount/copy/unmount/health checks and cannot recover acceptance after
   a cleanup error. Missing captured networking rejects enabled steps upfront.
2. Network setup failure keeps its individually owned creator ahead of global
   reaping; command rejection proceeds only after that cleanup. Command failure
   retry returns rejected storage, never an ExecutedStorage. Both carapace and
   bootstrap failure handlers discard recovered snapshots and return the primary
   error even when cleanup completes. No cleanup retry resumes the stage loop.
3. Carapace derives one snapshot per directive, closes the DM target, detaches
   its loop backing, then seals and extends the verified chain. Thus no mounted
   mutable volume reaches sealing on the accepted production path. Earlier
   successful layers do not make a later failed stage publishable.
4. Bootstrap formats a snapshot over dm-zero and uses a distinct writable tool
   snapshot. Both roots stay paired through directives and cleanup. Tool writes
   are discarded; output closes/detaches and seals with the zero parent, without
   image import/conversion. Later carapace directives extend that output. A
   separate firmware tool image changes tools, not output-root ancestry.
5. Firmware tools receive the selected output root as CARAPACE where available,
   run on a disposable snapshot and extract both PMI and DTB only after commands
   finalize. Extraction pins confined regular files and copies them to RAM under
   budget checks. No tool scute is added to outputs. Captured firmware is accepted
   only after tool volume/chain cleanup and DTB finalization succeed.
6. Snapshot sealing consumes writable scratch returned after detachment. It checks
   header validity/chunk geometry, scans exception metadata to the terminator,
   rejects out-of-range/data-in-metadata references and truncates unused capacity.
   The wire format remains in devmap-snapshot. This is intentionally not an
   independent verifier of arbitrary malicious snapshot metadata: command code
   cannot access backing, and kernel snapshot health precedes detachment.
7. The same data stream feeds digest and verity generation with known length and
   parent salt. SealedScute exposes read/seek views, not public writable/raw-FD
   access. It proves sealed bytes, not successful command execution: production
   callers must establish the lifecycle receipt first. Failure retains scratch
   for diagnosis; it does not yield a sealed output.
8. Stage coordinators require runtime.close even after assembly. Top-level pipeline
   closes the private context before CompletedImage publication. Inventory output
   checks precede creation of that private receipt. An assembly or inventory
   error cannot bypass these calls. Atomic publication implementation remains an
   M6 review requirement; this segment traces only its admission conditions.
9. Pipeline error conversion still renders several typed resource errors into
   strings, losing retry access at this boundary. The VM-disposal contract allows
   terminal rejection, but structured-error completeness remains part of the
   historical review reconciliation. No successful cleanup is inferred from an
   error return or a dropped owner. Low-level cleanup drops may leave resources
   requiring disposal, as already documented.
10. Style: stage selection is read-only; successive owned chains and reapers move
    through consuming transitions. Local loops rebind owners rather than expose
    mutable stage flags. Private CompletedImage/CompletedDirectives constructors
    prevent unrelated callers from manufacturing successful pipeline receipts.
    Broader duplicate coordinator setup/error conversion is not silently counted
    as resolved style work by this boundary review.

### UID/GID requirement needing reconciliation

All three coordinators currently select IdRange::new(100000, 65536) for both
UID and GID. SANDBOX-DESIGN.txt 4.2 calls for all supported image IDs, rejection
of unmapped IDs, and explicitly warns that the prototype range is not a universal
OS-image requirement. This review has not established a declared supported range,
input-filesystem ownership validation, or coverage for workloads needing IDs
outside it. Kernel rejection of an unmappable chown alone does not establish
handling of an inherited inode already owned outside the range. Treat this as an
open requirement/qualification item; do not claim universal UID/GID fidelity or
silently narrow the design. Resolve the supported mapping contract and inspect
inherited ownership/ACL behavior before a clean milestone pass.

### Pipeline evidence comparison

pipeline-evidence-comparison.json records exact archive paths. All eight selected
files match status-network-resolver-v1 and observation-blocked-descendant-v1.
The former exercises successful actual CLI publication after a network command;
the latter exercises lifecycle rejection/retention, not successful pipeline
publication. receiver-root-negative-v1 predates rejection-only cleanup API changes
in the bootstrap/carapace callers and execution module. No missing file is
represented as a source match. No new execution occurred in this segment.

Remaining cumulative assessment: historical findings, UID/GID contract, preservation
and current test coverage. Existing network/label qualification gaps remain;
M4 stays 0/3. This review does not advance M5/M6 or certify their complete scopes.


## Disposition and work queue

This attempt is NOT CLEAN. Source review found substantial lifecycle controls and
reconciled targeted execution evidence, but required scope remains unproven and
accepted historical actions remain open. M4 streak stays 0/3. Do not restart
these seven subset reviews merely to accumulate more prose; resolve the following
items, then perform a complete fresh adherence pass against the resulting source.

1. Resolve UID/GID mapping coverage (segment G): declare the supported contract
   consistently with OS-image requirements, handle inherited out-of-range owners
   and ACL IDs, and add benign disposable-VM boundary tests. Do not assume a
   prototype mapping covers all distro workloads.
2. Resolve network destination collisions rather than qualify only visible-route
   avoidance. Evaluate the proposed link-local alternative with actual forward/
   reply paths before changing allocation. Independently validate IPv6 and
   unsolicited inbound denial; use positive controls for connectivity.
3. Refresh current missing-label-policy and second-root negative controls. Preserve
   earlier failures and compare tested source explicitly. Qualify dangling and
   command-time replaced resolver targets; existing rootless helpers are narrower.
4. Work accepted style/API cleanup from historical-actions-current.json in coherent
   batches with appropriate tests. Some entries are outside M4 and remain tracked
   for M5–M7, not counted as lifecycle defects. In particular shared stage recovery
   and Runtime cleanup must preserve failure owners, diagnostics and rejection.
5. Finish current dispositions for pending historical actions and all original
   111 finding IDs. This attempt updates 46 previously accepted/open actions only;
   it does not reopen rejected findings without evidence or pretend pending ones
   have been resolved. Original ledgers remain immutable reference evidence.
6. Preserve capacity for testing without deleting unique artifacts. Original local
   archive rehashed successfully (preservation-check.json); later large /var/tmp
   assets and raw untracked review streams still need durable recovery accounting.
   Root policy/labels.bpf.c is superseded scoped-policy code, not the current
   environment policy, and remains a retirement/provenance obligation. No removal
   is authorized by a hash check of the original archive alone.
7. After fixes and current evidence, execute all full milestone adherence gates.
   M5/M6, representative distro boots, arm64/confidential hardware, CI and M8
   retirement retain their original requirements. Missing hardware qualification
   stays outstanding. This source review cannot substitute for them.

### Historical ledger findings

historical-actions-current.json preserves every original action/evidence field
and adds a current disposition or explicitly pending revalidation. It corrects
stale earlier dispositions for removed deadline-free entrypoints and typed DTB/
manifest errors, while identifying still-present accepted work (timeout parsing,
zstd features, repeated coordinator cleanup, public accessors and allocations).
No old "accept" label is treated as proof of implementation. Security hardening
suggestions remain distinct from demonstrated defects and binding requirements.

The previous goal turn made concrete progress through pipeline/source evidence.
This turn adds current-action reconciliation and verifies the original 4.8 GiB
archive hash. No production edit, build, VM execution, artifact deletion or full
milestone pass occurred. Disk capacity remains limited (~443 MiB observed).
