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

The current child-owner primitive can acquire an independent pidfd observation
before collecting exit status. The launcher must retain normal SIGCHLD zombie
semantics (no explicit SIG_IGN or SA_NOCLDWAIT) and exclusive child reaping;
those process-wide conditions keep a terminated child's PID from being reused
between inspection and pidfd acquisition. An already collected child is rejected.
The observer exposes only exit readiness, never signaling or namespace-entry
authority. Reaping and observing exit are distinct from proving descendant
quiescence. A future launch path must establish these signal/reaping prerequisites
before using this primitive; the existing API does not establish them itself.
See the conditions documented in
[pidfd_open(2)](https://man7.org/linux/man-pages/man2/pidfd_open.2.html).

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
