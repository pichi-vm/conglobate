# Replacement architecture

Milestone 2 draft. This specifies the new implementation, not the reference code.
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
uncertain partial operation. No successful output token is available on failure.

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

Proposed initial schema: filesystem.type, filesystem.size and optional
filesystem.properties.label. Initially type is ext4; additional drivers require
qualified backends. Size is a checked positive byte capacity using explicit
binary-unit strings; exact accepted units and block alignment must be specified
and tested before implementation. Unknown properties fail. Formatting flags,
executables and shell strings cannot be supplied by the recipe. Defaults must
be explicit in the effective spec and identified in inventory.

Formatter selection belongs to the trusted environment/backend. Pin or otherwise
stabilize the executable/dependency identity used; hashing a mutable path and
later executing its replacement is insufficient. Root formatting must receive
only the intended block-device authority. No host configuration discovery may
silently change formatting policy. Backend validation must reconcile requested
filesystem capacity with carapace's logical-device geometry before accepting it.
This geometry decision is still pending source inspection of the current APIs.

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

## Milestone 2 unresolved items

- Inspect current carapace/devmap APIs for geometry, zero-backed writable layer
  construction and sealing ownership; identify any dependency API gaps.
- Prototype state-specific resource transitions without unsafe application code,
  including ownership-preserving errors and scoped root borrowing.
- Specify supported size syntax, ext4 formatting defaults and formatter identity.
- Specify the supervisor/guardian policy recovery protocol and safe Rust BPF
  integration candidate; do not invent a destructor-based guarantee.
- Map the complete normative design to maintained tests and qualification inputs.

No adherence pass has been claimed for this draft. Architecture edits and findings
must settle before the three consecutive clean passes and milestone 3 admission.
