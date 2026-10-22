# Replacement architecture

Milestone 2 architecture candidate. This specifies the new implementation, not the reference code.
DESIGN.md and SANDBOX-DESIGN.txt remain normative. Changes to their trust model
require a user decision. Compile-time restrictions complement kernel checks;
they do not prove that external state cannot change or that destructors run.

## Ownership and modules

| Module | Owned or immutable domain types | Boundary |
| --- | --- | --- |
| request | BuildRequest, ContextPath, OutputDestination | Parse CLI without privileged effects; require recipe and lock paths |
| context | CapturedContext, ContextFile, ContextReader | Exact committed objects, confined private materialization, no command access |
| recipe | RecipeDocument, Recipe, Stage, Directive, CommandTimeout | Raw deserialization followed by validated immutable values |
| plan | BuildPlan, StagePlan, InputSelection, ToolSelection | Resolve inheritance/references and requirements before command admission |
| image | ResolvedImage, VerifiedCarapace, ImageComposition, FirmwarePair | Immutable component identities and retained ancestry; no raw mutable manifest API |
| storage | Workspace, Volume<S>, Layer<S>, StorageLease | Tmpfs, verified cache-backed chain, device mapping and writable storage ownership |
| filesystem | FilesystemSpec, Backend, FormatterIdentity, RootView | Validated formatter properties, supported kernel driver and idmapped view |
| execution | Step<S>, StepOutcome, RecoveryLease | Own all authority passed to commands and completion/teardown order |
| isolation | NamespaceLease, IdentityLease, ProcessDomain, LabelPolicy<S> | Kernel identity lifetimes, descendants, private mounts and scoped authorization |
| network | NetworkLease, ResolverInput | Disabled default, owned outbound NAT and confined temporary resolver injection |
| copy | CopyPlan, ContextReader, RootWriter | Explicit selected context files; no context mount; confined handles |
| firmware | FirmwareSelection, FirmwarePair, FirmwareContract | Extract both files after writers stop; validate before accepting replacement |
| transform | TransformPlan, PropertyPath, TypedValue, TransformReceipt | Pure checked DTB operations and explicit default selection |
| inventory | ExecutionReceipt, Inventory, EncodedInventory | Bind actual completed work, selected input paths and immutable outputs |
| artifact | Artifact<S>, BlobStore, PublishedArtifact | Staging, content validation and atomic no-replacement publication |

Modules need not become separate crates. Traits represent actual replaceable
behavior or state constraints; do not create a trait for every struct. External
crate selection must preserve application forbid(unsafe_code) and license rules.
The binary entrypoint delegates to an application object. Production free
functions require a specific justification; avoid util modules and static helper
namespaces that merely disguise procedural architecture.

## Immutable values and type transitions

Private fields protect invariants. Deserialize untrusted documents into raw
forms, then convert to validated values. Do not derive Deserialize for verified
runtime authority or finalized output types. Methods expose read-only views;
consuming transitions return replacement values. Use enums for runtime choices
and structs for values that must exist together, rather than option/boolean bags.

State traits are sealed. Empty marker types identify state; associated resource
payloads contain state-specific RAII owners. Prefer resource guards implementing
Drop and an outer typestate wrapper that can move its fields during conversion.
No unsafe field extraction, transmute, or manually asserted state transition.

- Volume<Unmounted> -> Volume<Mounted> -> Volume<Unmounted> controls mount ownership.
- Layer<Writable> -> Layer<Finalized> -> Layer<Sealed> controls writable storage.
  Unmounted alone does not prove flush success or absence of other writers.
- Step<Prepared> -> Step<Running> -> Step<Quiescent> -> Step<Completed> controls
  command admission and explicit teardown.
- LabelPolicy<Prepared> -> LabelPolicy<Installed> -> LabelPolicy<Removed> controls
  authorization. Step owns the installed policy; unrelated callers cannot remove it.
- Artifact<Staged> -> Artifact<Validated> -> Artifact<Published> controls output.

No public Clone on authority owners. Read-only descriptions and content IDs may
be cloned. Public handles must not permit recovering unrestricted writable FDs.
Borrowed root views cannot outlive their owner or overlap consuming unmount.
A process holding a descriptor is not represented by a Rust borrow: Step must
retain actual kernel/process leases until quiescence is independently established.

Consider invariant generative build/step brands only if ownership aggregates do
not adequately prevent cross-build mixing. Ordinary lifetime parameters alone
do not establish instance identity. Defer that complexity until an API experiment
shows a concrete benefit. Thread-affine setup guards must not be Send/Sync merely
because their fields happen to be; namespace mutation remains isolated to an
appropriate single-threaded helper. Immutable plans may be freely shared.

## Error and recovery ownership

Every fallible consuming transition describes the external state on failure.
Return the original typed owner only if its invariant remains known true.
Otherwise return a RecoveryLease owning the remaining resources and primary plus
cleanup diagnostics. Do not claim retryable mounted/unmounted state after an
uncertain partial operation. No successful output token is available on failure. Quiescent means no remaining
command authority, not command success. Step<Quiescent> retains a StepOutcome;
teardown produces Step<Completed> only for successful execution and successful
cleanup. Otherwise it returns failure/diagnostics without a completion receipt.
An exit-zero shell with surviving descendants still requires containment cleanup;
a timed-out, cancelled or failed command never becomes successful through cleanup.

Drop is best-effort cancellation/rollback, never a successful finalization proof.
No guarantee depends on unwinding, forgetting being impossible, or a destructor
running after process death. Explicit fallible methods establish completion;
external supervision and kernel-held leases cover crashes. Keep ownership of
unresolved mappings, policy pins and identities until recovery completes.

The step aggregate, not incidental field-drop order, enforces:
1. Stop admission and stop/reap every descendant, including daemonized commands.
2. Prove quiescence under the containment mechanism.
3. Disable/remove scoped labeling policy while namespace and mount identities
   remain held. Never mutate policy while a command can be between its two hooks.
4. Remove temporary resolver/network/runtime attachments, preserving image state.
5. Flush and cleanly unmount all assigned filesystems; inspect storage errors.
6. Release kernel resources and permit finalization, sealing or the next step.

Partial setup needs acquisition guards before each fallible operation. Policy
installation starts disabled and installs/pins every guard before enabling.
Unknown startup process scope or failed quiescence retains protection and rejects
publication. The guardian/supervisor death protocol requires explicit qualification,
not inference from loader-death experiments. Fixed internal deadlines bound
acceptance/waiting but cannot guarantee recovery from uninterruptible kernel I/O.

The initial command-domain primitive creates a fresh private cgroup-v2 directory
under a protected root-owned parent. It does not provision controllers or resource
limits. Its ownership survives userspace Drop in the kernel hierarchy, while
explicit cancellation writes cgroup.kill, waits for cgroup.events populated=0,
and removes only that directory. Failed cancellation/removal retains the domain;
failed construction after creation retains a removable target. Domain creation
requires a RecoveryDirectory. The domain/target internally shares its directory
lease, keeping the cooperative recovery lock held after the facade is dropped.
Registration, cancellation and target removal verify the deployment namespace
through that shared lease. No public lease clone, raw descriptor or early unlock
is exposed. No recursive cleanup or arbitrary-existing-cgroup kill API is exposed.
Domain records now use canonical bounded CBOR
`[1, name, parent-device, parent-inode, device, inode]`. Saving verifies live path
identity and requires the same recovery lease that owns the domain. Protected
record loading returns data only; recovery loads that record under the lease,
verifies the supplied parent's identity and the exact cgroup directory identity,
permissions and domain type, then retains the lease in the recovered owner.
Wrong parents, missing targets and substituted directories fail. Removing the
record requires a matching stored description, the original parent identity and
an absent target. These records do not create a complete transactional build lease
or establish that another owner has stopped; the aggregate must enforce exclusive
recovery and preserve all policy/mount identities through descendant cleanup.

Registration now requires DomainRegistration, a borrowed capability obtained
only after comparing the live domain with its protected recovery record under
the same lease. Bare CommandDomain owners are not accepted. The capability
rechecks that match immediately before yielding the private registration file;
its borrow prevents consuming the domain for cancellation during exchange. It
uses an internal shared lease view, so dropping the public directory facade does
not bypass or prevent the check. This enforces domain-record publication, not
persistence of the complete build or readiness of the command sandbox.

RecordedDomain now owns the cgroup and its published description while borrowing
the recovery directory. Creation retains the raw domain on publication failure.
Cancellation consumes the owner; failure returns DomainCancellation, which exposes
only retry, and success returns DomainRecordCleanup. Recovery likewise returns a
cleanup-only DomainCancellation. Record cleanup consumes its owner and retains it
on failure. A relocated parent can be supplied for retry, with identity checked
against the record. Post-unlink failures remain errors even if the record is now
absent. These consuming ownership transitions enforce domain-before-record cleanup;
they do not certify reaping, policy removal, image validity or complete build recovery.

ContainedHelper now combines that recorded domain with its identified helper
and registered supervision channel. RecordedDomain::launch constructs the pair
through the persisted registration gate; startup failures retain any child and
a cleanup-only domain. Cancellation first closes the channel, then consumes the
domain into HelperReaping. Only collecting the helper status yields ReapedHelper,
and only record cleanup releases that diagnostic status. Each failed transition
retains its owner for retry. Private fields prevent mixing unrelated helper/domain
owners. This is still trusted-helper containment, not recipe admission: it owns
neither mounts nor policy, does not reap all adopted descendants, and produces no
successful-step or image-finalization receipt. Drop is fallback, not completion.

The registration exchange places a trusted helper in this domain before its
first fork: the parent transfers a freshly opened cgroup.procs descriptor, the
helper writes its own membership (0), closes the descriptor and acknowledges.
Only the explicit exchange consumes these control messages. Sealed Registered
state records that acknowledgement; it is not sandbox readiness or admission.
The helper must execute this at dedicated single-threaded startup, with no prior
descendants; a task-count check diagnoses misuse but cannot prevent trusted-code
races. The parent never reopens or writes a numeric child PID. Domain and child
ownership remain with their respective callers on exchange failure.
The aggregate must still persist domain identity before registration and hold
recipe execution until all isolation and supervision gates are complete. The supervisor/guardian must remain outside the domain, commands
must have no cgroup control access, and admission must be stopped before teardown.
Empty membership is not a reaping, policy-removal or successful-step receipt.
Existing preserved guardian experiments and the new disposable-VM primitive test
support this mechanism, not the complete production supervision protocol.
See the [kernel cgroup-v2 documentation](https://docs.kernel.org/admin-guide/cgroup-v2.html)
for inheritance, migration and subtree-kill semantics.

## Pipeline owners

BuildPlan is immutable. A stage consumes ImageComposition and returns a new
composition plus immutable execution receipts; references to inherited blobs
need not copy their bytes. A coordinator sequences stage methods without owning
all low-level setup logic or mutating a shared manifest throughout the build.

BootstrapSession owns a disposable tool layer and a new writable output layer
over dm-zero. Trusted formatting happens through the assigned output device.
Each command gets / and /mnt views, and policy authorizes exactly both intended
filesystems. COPY uses the same root layout. No rootfs file import/conversion.
Successful teardown seals the output layer and discards tool changes.

CarapaceSession derives output layers while preserving every inherited component.
FirmwareSession owns one disposable tool root, with no special output /mnt. It
extracts declared paths only after quiescence and returns a validated pair.
TransformPlan operates without a command sandbox. Missing vs empty sections,
CARAPACE availability, regex no-match semantics and default selection follow the
design. Recipe key order never controls execution.

## Filesystem declaration and environment contract

Initial schema: `filesystem.type: ext4`, required `filesystem.size`, optional
`filesystem.properties.label` (default empty, at most 16 UTF-8 bytes, no NUL).
Size syntax is a positive decimal integer immediately followed by B, KiB, MiB
or GiB. No fractional values, signs, whitespace or implicit units. Check
multiplication overflow, 4096-byte alignment, and a 16 MiB..64 GiB supported
range. The lower bound is the initial journaled-ext4 backend contract; unsupported
sizes fail early. Additional filesystem types/properties need qualified backends.

The requested size is FILESYSTEM capacity, not a new carapace geometry. The
pinned carapace API uses a 64 GiB zero origin. Create that geometry unchanged and
format exactly size/4096 blocks with an explicit 4096-byte mke2fs block size.
Unused logical space remains zero-backed. No image conversion or second full
volume is created. Record both filesystem capacity and logical device capacity
in effective execution metadata. Scute wire parsing belongs to devmap-snapshot;
verified read-only chain assembly belongs to carapace. Conglobate owns writable
activation/finalization and uses those APIs rather than reimplementing formats.

Formatter selection belongs to the trusted environment/backend. Capture the
formatter executable and dependencies into private immutable execution inputs,
identify their bytes, and execute that captured tool, not its original pathname.
Provide a builder-owned mke2fs configuration, clear the environment and explicitly
select ext4, block size, label, root ownership 0:0, nodiscard and eager journal/
inode initialization. A versioned backend profile supplies feature/inode/reserved-
space defaults; record the profile bytes and effective arguments. Generated UUID
and other output metadata are output bytes; no reproducibility guarantee is
claimed. Commands cannot select arbitrary executables, flags or shell fragments.
The formatter runs in trusted setup with only the assigned device and captured
runtime, no context or image scripts. Complete executable/dependency capture and
formatter-failure qualification are M4/M5 tests, not claimed prototype coverage.

BPF authorization must support an explicit set of superblock owner/device pairs,
separate from the command user namespace and mapped-root identity. No wildcard
for all filesystems owned by the supervisor. Bind both bootstrap filesystems,
pin their lifetime, match the exact security.selinux name and preserve other
LSM denials. No SYS_ADMIN fallback. Runtime mount/device/filter restrictions
remain necessary even though labeling no longer needs that capability.

## Tests and evidence structure

New tests live in maintained test modules/integration fixtures, not in legacy
validation. Use methods for fixture objects; test entry functions are justified
harness integration. Compile-fail tests cover key ownership restrictions, but
avoid mirroring every method signature. Runtime tests validate external effects.

| Requirement | Evidence required before milestone acceptance |
| --- | --- |
| Input integrity | Wrong commit/object/lock digest, escaping path and missing-cache negatives; exact inventory paths/bytes |
| Recipe correctness | Every stage combination, omitted/empty distinctions, from rules, transform types/defaults |
| Owned lifecycle | Partial acquisition at each boundary; cleanup failure retains recovery; no publishable token |
| Command isolation | No context/device/control access; five capability sets; private namespace/FD checks |
| Scoped LSM | Exact-name regression, both authorized mounts, denied third mount/nonroot/other namespace, policy-off denial |
| Teardown | Live descendants, timeout/cancellation, launcher/guardian/supervisor death, blocked I/O, concurrent builds |
| Bootstrap | Actual zero-backed chain, formatter geometry, / plus /mnt packages, metadata persistence, direct sealing |
| Filesystem fidelity | UID/GID, ACL access/defaults, file capabilities, SELinux labels and target enforcing boot |
| Network | Disabled default, outbound NAT/replies, explicit resolver including confined symlinks and cleanup |
| Outputs | Paired firmware validation, final DTB, deterministic CBOR, inherited blobs, OCI/ORAS, atomic failure cases |
| Platforms | amd64/arm64 and TDX/SEV-SNP/CCA evidence; unavailable environments remain outstanding |

## Recovery anchor and protocol

Before any command admission, create a root-only recovery anchor in a trusted
mount namespace whose lifetime is independent of build processes. The default
host/VM deployment uses the persistent initial mount namespace. The anchor
retains bind-mounted namespace handles, assigned filesystem mounts/scratch and
private bpffs links/config. Verify this prerequisite; a private namespace that
will disappear with the builder is insufficient. Commands must not inherit the
anchor path, namespace handle or controller channel. The binary does not require
a particular init system or attestation service.

A dedicated guardian owns a per-build lease and watches the supervisor through
stable process identity and a private control channel. It retains every identity
and mapping needed for cleanup. Command creation registers containment before
release to execute; an unregistered process cannot run recipe code. Supervisor
loss triggers guardian cancellation and ordered teardown. Guardian loss triggers
supervisor cancellation. Concurrent loss leaves the kernel-held anchor in place,
not a policy with reusable unpinned identifiers. Recovery on a later invocation
validates the lease, shuts down remaining command domains and removes policy
before dropping anchored identities. Unknown or incomplete ownership is
quarantined, not guessed from a numeric PID. Publication requires an intact
supervision protocol and all completion receipts. This is the target protocol;
its death/race/failure cases are mandatory M4 qualification.

The anchor addresses a security issue independent of resource leaks: deleting
namespaces while leaving global LSM authorization pinned can authorize later
identity reuse. No cleanup fallback may make that tradeoff. Namespace/mount
anchors can persist after a failed build until a trusted recovery pass succeeds.
Exact lease serialization and readiness/acknowledgement messages are implementation
choices subject to this protocol and fault-injection tests.

The NamespacePin primitive bind-mounts an owned nsfs descriptor beneath a
root-owned mode-0700 tmpfs parent. Its immutable record identifies the generated
name and namespace inode/device. Dropping the userspace owner leaves the kernel
mount for recovery; reopening verifies nsfs identity without following a final
symlink. Explicit release distinguishes a still-mounted pin from an unmounted
target needing unlink. This primitive assumes serialized parent access and does
not establish the deployment init lifetime contract by itself. A future
anchor aggregate must persist a complete lease and own all
namespace/filesystem/policy identities before enabling authorization. Namespace
pins alone are not a crash-recovery or command-admission receipt.

Creation and recovery now require a RecoveryDirectory lease: a nonblocking
exclusive flock on the root-only tmpfs directory descriptor. Pins and unmounted
cleanup targets share that open description internally, so dropping the directory
facade cannot unlock their live resources. A separately opened recovery owner is
excluded until the final reference closes. Descriptor-relative access survives
directory rename. This is cooperative locking, not protection against privileged
protocol bypass or replacement of the directory through its parent.

Namespace and domain control records share bounded, private, no-replacement
file storage. Namespace records use deterministic CBOR `[1, name, nsfs-device, inode]`, bounded
at 256 bytes. Decoding validates canonical encoding, generated-name syntax and
identity ranges; it grants no ownership or cleanup authority. Publication checks
the mounted identity, writes and syncs a private temporary file, publishes without
replacement, then syncs the directory. An error after publication can leave the
complete record: callers must fail and inspect under the lease. Reading rejects
symlinks, non-regular/non-private files, oversized/corrupt records and mismatched
names. Record deletion requires an exact stored match and an absent pin target;
policy/containment teardown remains the enclosing owner's prerequisite. This is
RAM-backed recovery across process loss, not across VM loss. Complete build-lease
serialization remains required.

RecoveryDirectory now captures the deployment init's mount namespace descriptor
and pidfd, shared with every pin/target through the directory lease. Opening a
lease rejects PID 1 itself, mismatched procfs PID views, different PID namespaces
and a current mount namespace different from init's. Pin creation, verification,
release and record operations recheck the current/init namespace identities and
init's live pidfd before mutation. A private helper cannot use a carried lease to
mutate its copied mount tree as though it were the persistent anchor. A failed
release retains original pin ownership for restoration/recovery.

This is a deployment-relative check, not proof of the machine's initial PID/user
namespace or protection against init changing namespaces after a check. Deployment
must keep its trusted init alive in that mount namespace independently of builds;
containers with a disposable init cannot satisfy the lifetime requirement merely
by passing these checks. Production guardian/admission still must enforce the
complete protocol. Unmounted target unlink remains descriptor-relative and does
not require namespace entry or release any namespace identity.

The helper identity handoff uses a private close-on-exec sequence-packet socket
and one deterministic CBOR message `[1, 0]` (version, identity-offer opcode),
with exactly one SCM_RIGHTS descriptor. The trusted helper opens its own pidfd
while alive and sends it; the receiver never reopens a numeric child PID.
Received descriptors are close-on-exec and owned immediately. Truncated messages,
unknown versions/opcodes, missing/extra descriptors, peer loss and deadlines
fail the exchange. This channel belongs exclusively to trusted helper startup;
it is never made available to recipe commands. Receipt establishes identity
only, not readiness or command admission. The eventual launcher must retain
cleanup ownership on exchange failure and complete registration/acknowledgement
before releasing any recipe execution. Normal child wait/reaping semantics are
still required for child cleanup; this handoff does not establish them.
The trusted launch primitive owns the spawned child before receiving identity.
Pre-spawn errors have no child owner; exchange errors return the pending child
for explicit recovery. A successful exchange yields Process<Identified>, whose
state payload retains the pidfd for explicit and destructor termination, with
no numeric-PID fallback after identification. Wait failures preserve that state
and both owners. Process<Reaped> remains solely a collected exit status. The
caller supplies trusted helper code and startup configuration; this primitive
does not launch arbitrary recipe commands or replace containment/admission gates.
Supervision<Unpaired> -> Supervision<Paired> adds a mutual startup exchange:
both trusted processes send their own pidfd over the private channel before
receiving the peer's identity under a single deadline. Paired state owns the peer
observation and the retained CLOEXEC control socket. Normal Drop shuts down the
endpoint even if a transient inherited descriptor remains; pidfd observation
independently detects process exit without destructors. A bounded poll observes
both mechanisms. Unexpected payloads outside the explicit registration exchange are errors, not
accepted control commands. Observation peeks rather than consumes those payloads, so a
second check cannot turn a protocol violation into a healthy observation. This initial protocol has no readiness or admission message.

Process<Identified>::launch_supervised returns both the identified child owner
and the paired supervision channel. Exchange/descriptor-duplication failures
cancel the channel and retain the started child for cleanup. Both successful
owners independently retain the same kernel process identity. A sampled live
peer is not a lifetime guarantee, and observed loss is not descendant quiescence.
GuardianWatch now borrows a paired supervisor channel and a registered helper
channel, polling both sockets and both pidfds under one deadline. Its sampled
ShutdownTrigger distinguishes supervisor/helper/both loss for diagnostics; it
neither orders exits nor certifies command success or quiescence. Protocol and
I/O errors must also start failed-build cancellation. It deliberately requires a
registered helper; setup-phase monitoring remains a separate obligation.
RegisteredGuardian now owns the upstream paired channel and ContainedHelper.
Its consuming supervise_until waits for either peer, then attempts domain
cancellation on every outcome, including timeout and protocol/I/O error. The
must-use GuardianStop preserves observation and cancellation outcomes separately;
success retains HelperReaping, failure retains cleanup-only HelperCancellation.
Neither path returns a registered helper or guardian. Upstream closure is not a
completion acknowledgement. This registered-phase owner still needs integration
with the independent guardian process, setup monitoring, persistent mount/policy
resources, adopted-descendant reaping and final completion acknowledgements.

See [pidfd_open(2)](https://man7.org/linux/man-pages/man2/pidfd_open.2.html)
for the parent-side PID lookup constraints this identity handoff avoids.

## Safe integration and dependency evidence

- carapace 99caf79: Chain::assemble uses a fixed 64 GiB zero target and returns
  a read-only device File with deferred removal. Its public API rejects empty
  chains; create the bootstrap zero/writable activation through devmap APIs.
- devmap 2679426: snapshot Header/Exception and verity types supply wire parsing
  and sealing primitives. A COW's clean kernel-produced state still needs checked
  length/status/finalization; it is not trusted merely because a command exited.
- Cached rustix 1.1.5 has safe open_tree/move_mount/pidfd APIs but lists
  mount_setattr as unimplemented. Cached nix 0.30 also lacks that wrapper.
  Treat this as a dependency gap: qualify an existing safe wrapper or contribute
  a general safe API upstream before native idmapping integration. Do not hide
  application FFI in a locally named dependency to bypass forbid(unsafe_code).
  M4 follow-up: upstream rustix proposal 1630, revision 8bda151d, supplies a
  borrowed-descriptor MountAttr and internal structure sizing. Both backends
  passed the disposable-VM ext4 idmap/metadata/cleanup fixture recorded in
  validation/rewrite-mount-api. Production now pins that revision; it is not
  represented as a released/merged API. Production lifecycle ownership and
  broader sandbox qualification remain required.
- libbpf-rs provides owned links with pin/unpin and byte-slice map updates; it is
  the candidate safe userspace LSM interface. Explicit byte encoding avoids
  application unsafe POD implementations. Dependency/version/license and actual
  hook attachment remain M4 checks. The BPF policy is separately reviewed trusted
  policy code; safe Rust in userspace does not make BPF semantics automatically safe.
  M4 follow-up: libbpf-rs/libbpf-cargo 0.27.2 and libbpf-sys 1.7.0 are now locked.
  The build embeds the policy object without generating unsafe Rust skeletons.
  Prepared -> LoadedDisabled validates the embedded schema, loads programs and
  initializes targets while checking config remains zero. No links or pins are
  created. VM tests cover successful load/Drop and permission-failure cleanup;
  activation, anchoring and link-lifetime ownership remain unimplemented.
  A subsequent disabled-pinning owner now attaches/pins the complete hook set
  under a private bpffs directory, returning partial resources on failure and
  retaining cleanup authority on error. Explicit close, Drop and pin-collision
  recovery pass in the VM. Config remains zero; this owner cannot enable policy.
  Its directory descriptors are not process-death-resistant namespace anchors.
- The architecture probe in validation/rewrite-architecture/ compiles state-specific
  RAII payloads and an ownership-preserving error without unsafe code. Borrow and
  invalid-state negative examples fail with E0505 and E0599. It models API legality,
  not mount behavior or kernel recovery.

Primary references inspected:
- https://man7.org/linux/man-pages/man8/mke2fs.8.html (explicit block count/config).
- https://docs.rs/libbpf-rs/latest/libbpf_rs/struct.Link.html (pinning/ownership).
- https://docs.rs/libbpf-rs/latest/libbpf_rs/trait.MapCore.html (byte map interface).
- Pinned carapace src/lib.rs and src/assemble.rs; cached rustix mount/process APIs.

## Acceptance boundaries

Architecture fixes the invariants, schema and ownership/recovery protocol. It
identifies dependency gaps explicitly rather than claiming implementations exist.
M3 handles pure inputs/planning. M4 must prove the actual safe syscall/policy,
process-containment and anchor protocol. M5 must prove direct formatted snapshot
behavior and dual mounts. Failure of those proofs reopens the architecture gate.
Hardware and distro qualification remain M7 requirements; no absence of hardware
can be recorded as a pass. Public library APIs may evolve within these boundaries
without adopting mutable option bags or weakening the safety constraints.
