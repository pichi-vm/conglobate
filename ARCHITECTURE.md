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

- `Volume<Unmounted> -> Volume<Mounted> -> Volume<Unmounted>` controls mount
  ownership. An unmounted value alone does not prove flush success or absence
  of other writers. Explicit snapshot closure and loop detachment return the
  scratch owner; `Scratch::seal_snapshot` produces a `SealedScute` only after
  validating the persistent snapshot extent and constructing its verity data.
- `Step<Ready>::admit_until` produces `Step<Running>`. Activation failure retains
  Ready; failed admission retains Cancelling. Ready, Running or Cancelling can
  enter cleanup through `cancel_until`, producing Reaping; `reap_until` produces
  Draining, `drain_until` produces Quiescent, `finish_record` produces Finishing,
  and `disable` returns StepCompletion. Failed cancellation retains Cancelling;
  later failures retain their input state. `teardown` also closes policy and
  identity pins, yielding TeardownCompletion under the original fixed deadline.
  Storage finalization and sealing still follow. A normal shell exit does not
  bypass cancellation or descendant drain.
- `LabelPolicy<Prepared> -> LabelPolicy<LoadedDisabled> ->
  LabelPolicy<PinnedDisabled> -> LabelPolicy<BoundDisabled>` constructs and binds
  disabled authorization. Step owns activation, retains the bound policy through
  quiescence, and disables it before returning ownership. Policy closure returns
  namespace/filesystem identity pins for separate cleanup; raw policy owners
  expose no enable operation.
- `Supervision<Unpaired> -> Supervision<Paired> -> Supervision<Registered> ->
  Supervision<Admitted>` separates identity exchange, domain registration and
  command admission. Peer observation alone is not admission authority.
- `StagedDirectory` owns low-level output storage. `AssembledImage` binds the
  component layout, and the private `CompletedImage` couples it to validated
  inventory after the selected pipeline completes. Publication consumes that
  owner only after private context cleanup. Raw staging publication by itself
  is not a successful-build receipt.

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
independent VM shutdown handles abrupt supervisor death. Keep ownership of
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
publication. Supervisor death and environment-driven VM shutdown require explicit
qualification, not inference from loader-death experiments. Fixed internal deadlines bound
acceptance/waiting but cannot guarantee recovery from uninterruptible kernel I/O.

The current integration gaps and extraction order are recorded in
[M4-INTEGRATION.md](M4-INTEGRATION.md). The validation executable still assembles
the command path; it is not the production build coordinator.

The existing domain, process, mount and disabled-policy primitives are reference
inputs for the integrated executor. Reuse their typed ownership and bounded
cancellation where appropriate, but persistent recovery records and a guardian
are not prerequisites for command startup under the dedicated-VM contract.
Earlier implementation details and the superseded recovery protocol are preserved
in preservation/pre-dedicated-vm-contract/ARCHITECTURE.md. They are historical
mechanisms, not additional current requirements.

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
carapace v1 format uses a 64 GiB zero origin. Create that geometry unchanged and
format exactly size/4096 blocks with an explicit 4096-byte mke2fs block size.
Unused logical space remains zero-backed. No image conversion or second full
volume is created. Record both filesystem capacity and logical device capacity
in effective execution metadata. Scute wire parsing belongs to devmap-snapshot;
carapace defines the read-only chain layout and geometry. The accepted M4
integration keeps read-only activation and its explicit cleanup owners in
conglobate's storage module, using devmap/lodown. The inspected carapace
Chain::assemble API returns a read-only File with deferred removal, which cannot
supply conglobate's fallible, ownership-preserving partial-setup and teardown
contract. Conglobate also owns writable activation/finalization. Both paths use
ecosystem wire/target APIs rather than duplicating codecs. This is the integration
refinement recorded in REIMPLEMENTATION.md's ReadOnlyChain work and accepted by
preservation/MILESTONE-4-ADHERENCE.md; it does not change the carapace format or
introduce a dependency on the pichi binary/repository.

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
| Teardown | Live descendants, timeout/cancellation, helper death, supervisor death plus VM shutdown, blocked I/O |
| Bootstrap | Actual zero-backed chain, formatter geometry, / plus /mnt packages, metadata persistence, direct sealing |
| Filesystem fidelity | UID/GID, ACL access/defaults, file capabilities, SELinux labels and target enforcing boot |
| Network | Disabled default, outbound NAT/replies, explicit resolver including confined symlinks and cleanup |
| Outputs | Paired firmware validation, final DTB, deterministic CBOR, inherited blobs, OCI/ORAS, atomic failure cases |
| Platforms | amd64/arm64 and TDX/SEV-SNP/CCA evidence; unavailable environments remain outstanding |

## Dedicated-VM lifecycle

Conglobate is the sole build workflow in a disposable VM. An independent
execution-environment controller shuts down the VM if conglobate crashes or exits
unexpectedly; the binary has no pichi/VM transport dependency. The controller must
not launch another build or permit abandoned resources to be reused in that VM.
This contract replaces the separate guardian and cross-invocation recovery design.

The main conglobate process owns image storage, policy, command containment and
publication. A trusted setup helper establishes each command's isolation and
starts its untrusted payload. Namespace init reaps command descendants. No second
recovery supervisor or serialized resume protocol is needed. This does not remove
bounded cancellation, normal error cleanup, subreaping where needed, or explicit
fallible teardown. Destructors remain best effort, never completion evidence.

The step owner retains namespace, superblock and policy lifetimes together until
commands stop and authorization is removed. On abrupt supervisor death, the
remaining lifetime arrangement must remain safe until the environment stops the
VM. VM shutdown is not instantaneous and does not justify partial guard removal
or unsafe identity reuse while commands still execute. The integrated execution
path must establish that behavior before enabling the policy. If cleanup cannot
finish while conglobate lives, fail and require disposal; never advance or publish.

Validation must include actual supervisor termination and externally triggered VM
shutdown, with no successful artifact accepted. Test the deployed environment's
contract rather than assuming a signal handler or destructor will enforce it.
Independent VMs provide build concurrency; persistent shared-host execution is
not covered. Preserved guardian/anchor experiments remain useful reference data,
but do not impose an implementation requirement.

## Safe integration and dependency evidence

- Inspected reference carapace 99caf79 (not an active crate dependency):
  Chain::assemble uses a fixed 64 GiB zero target and returns a read-only device
  File with deferred removal. Its public API rejects empty chains. The M4
  integration therefore owns read-only stack activation/cleanup with devmap and
  lodown, retaining each dependency until upper mappings are removed. Bootstrap
  zero/writable activation also uses devmap APIs. See the integration boundary
  above and the accepted M4 source/evidence record.
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
- Preserved reference carapace src/lib.rs and src/assemble.rs; cached rustix mount/process APIs.

## Acceptance boundaries

Architecture fixes the invariants, schema and ownership/recovery protocol. It
identifies dependency gaps explicitly rather than claiming implementations exist.
M3 handles pure inputs/planning. M4 must prove the actual safe syscall/policy,
process containment and dedicated-VM shutdown contract. M5 must prove direct formatted snapshot
behavior and dual mounts. Failure of those proofs reopens the architecture gate.
Hardware and distro qualification remain M7 requirements; no absence of hardware
can be recorded as a pass. Public library APIs may evolve within these boundaries
without adopting mutable option bags or weakening the safety constraints.
