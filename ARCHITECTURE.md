# Replacement architecture

Milestone 2 architecture candidate. This specifies the new implementation, not the reference code.
DESIGN.md and SANDBOX-DESIGN.txt remain normative. Changes to their trust model
require a user decision. Compile-time restrictions complement kernel checks;
they do not prove that external state cannot change or that destructors run.

## Ownership and modules

### Registration-free lifecycle revision (2026-10-23)

This architecture incorporates the fixed environment-owned labeling policy.
The superseded scoped-policy architecture is preserved in
[preservation/registration-free-architecture-c1073b9](preservation/registration-free-architecture-c1073b9).

The environment owns the fixed labeling exception for the VM lifetime.
Conglobate owns neither its program object nor map/link/pin handles. The production policy module and BPF build dependencies have been removed.
Historical assets, loaders and validation fixtures remain recoverable.

`CommandSandbox` prepares mapped namespace ownership and assigned storage.
`Step<S, R>` retains those owners through cgroup containment, admission,
cancellation, reaping and descendant drain. A namespace handle belongs to the
isolation module, not a `LabelPolicy` wrapper. Storage handoff takes that handle
directly; the owned volume determines which mount is transferred. No policy
scope/map lookup is required to prove that ownership. Remove filesystem pins
used only to prevent policy target identity reuse.

Sealed `ExecutionStorage` implementations derive every transferred `VolumeView`
from the storage retained by this `Step`, using its private root descriptor and
mount-namespace check. Bootstrap derives both views from its retained tool/output
owners. No caller may substitute an arbitrary root FD at this boundary. Retain
tests for this ownership invariant and confined receiver mount attachment.
The old `namespace_for_root` check reconciled independently supplied policy pins
and volumes; removing that policy owner removes the need for the check, not the
requirement that transferred mounts belong to the step.

Before recipe execution, the existing trusted leaf checks labeling after
`GatedRootSetup::restrict` applies command capabilities and seccomp, and before
`ExecutionEndpoint::ready_and_wait` reports readiness. A small owning probe
uses an anonymous `O_TMPFILE|O_EXCL|O_CLOEXEC` inode on each assigned filesystem,
performs descriptor-based label set/read/remove, and closes before readiness.
No named image path, new process, policy handle or extra admission channel is
needed. Supply the assigned-root layout through the existing trusted setup
input; do not discover bootstrap from an image-controlled `/mnt` entry.
Root selection must use confined, no-symlink handles for the actual assigned
mounts. A successful root-supervisor write is not evidence; neither is a named
bpffs pin. A failed check enters the same bounded setup-failure cleanup path.
Anonymous inode allocation can still change filesystem metadata; this is not
a claim of byte-for-byte neutrality. Test both bootstrap roots, missing-policy
failure and descriptor closure under the production filter. The mechanism fixture alone does not qualify production integration.
Single-root positive/negative and two-root bootstrap CLI regressions are recorded
in preservation/m7-review/label-unpinned-{on,off}-v1 and bootstrap-unpinned-v1;
broad failure, confinement and platform qualification remain outstanding.

The receiver attaches its mapped root over the existing supervisor-owned runtime
volumes directory only in its private mount namespace. It must not create a
shared attachment directory whose deletion depends on its destructor running.
Timeout cancellation can kill every contained helper. Private mount teardown
then removes the overlay without leaving shared directory entries; supervisor
storage owners retain the underlying volumes until descendant quiescence.
Normal helper completion explicitly unmounts its views. No recursive cleanup
sweep or additional tmpfs is required.

After descendant drain, release process-domain records and namespace resources,
then unmount/finalize storage. There is no activation, disable, policy-close,
installation-retry or policy-identity-release state. Cgroup joining remains a
distinct protocol represented by `Joined`, including its acknowledgment and
descriptor closure. Errors continue to retain
resource owners, and unfinished steps cannot yield publication authority.

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
| isolation | UserNamespace<Mapped>, AssignedRoots, LabelProbe | Mapped identity, private mounts and restricted labeling admission |
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
- `Step<Ready>::admit_until` produces `Step<Running>`; failed admission retains
  Cancelling. Ready, Running or Cancelling enters cleanup through `cancel_until`,
  producing Reaping; `reap_until` produces Draining, `drain_until` produces
  Quiescent, `finish_record` produces Finishing, and infallible `finish` returns
  StepCompletion. Failed cancellation retains Cancelling; subsequent fallible
  transitions retain their input state. `teardown` drives this sequence under
  the fixed cleanup deadline, then drops the mapped namespace descriptor.
  TeardownCompletion retains storage for explicit finalization and sealing.
  A normal shell exit does not bypass cancellation or descendant drain.
- `Supervision<Unpaired> -> Supervision<Paired> -> Supervision<Joined> ->
  Supervision<Admitted>` separates identity exchange, cgroup joining and command
  admission. Peer observation alone is not admission authority.
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
teardown produces storage ownership and the command outcome, not a successful
build receipt. The enclosing execution/finalization owner checks command success
and clean storage finalization before permitting the pipeline to continue.
An exit-zero shell with surviving descendants still requires containment cleanup;
a timed-out, cancelled or failed command never becomes successful through cleanup.

Drop is best-effort cancellation/rollback, never a successful finalization proof.
No guarantee depends on unwinding, forgetting being impossible, or a destructor
running after process death. Explicit fallible methods establish completion;
independent VM shutdown handles abrupt supervisor death. Keep ownership of
unresolved mappings, namespaces and storage until recovery completes.

The step aggregate, not incidental field-drop order, enforces:
1. Stop admission and stop/reap every descendant, including daemonized commands.
2. Prove quiescence under the containment mechanism.
3. Finish process-domain records and release command namespace ownership.
   The environment labeling exception remains installed until VM shutdown.
4. Remove temporary resolver/network/runtime attachments, preserving image state.
5. Flush and cleanly unmount all assigned filesystems; inspect storage errors.
6. Release kernel resources and permit finalization, sealing or the next step.

Partial setup needs acquisition guards before each fallible operation.
Unknown startup process scope or failed quiescence retains protection and rejects
publication. Supervisor death and environment-driven VM shutdown require explicit
qualification, not inference from loader-death experiments. Fixed internal deadlines bound
acceptance/waiting but cannot guarantee recovery from uninterruptible kernel I/O.

The production build coordinator owns the integrated execution path. Historical
extraction findings are preserved in
[preservation/registration-free-architecture-c1073b9/M4-INTEGRATION.md](preservation/registration-free-architecture-c1073b9/M4-INTEGRATION.md).
The old validation executable still references retired policy APIs and must be
migrated without losing failure coverage. Its historical passes do not qualify
the new lifecycle. Current gate status and outstanding work are in REIMPLEMENTATION.md.

## Pipeline owners

BuildPlan is immutable. A stage consumes ImageComposition and returns a new
composition plus immutable execution receipts; references to inherited blobs
need not copy their bytes. A coordinator sequences stage methods without owning
all low-level setup logic or mutating a shared manifest throughout the build.

BootstrapSession owns a disposable tool layer and a new writable output layer
over dm-zero. Trusted formatting happens through the assigned output device.
Each command gets / and /mnt views; restricted setup validates labeling on both
using the environment exception. Mount and descriptor confinement isolate them
from supervisor resources. COPY uses the same root layout. No rootfs file import/conversion.
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

The environment exception matches exactly security.selinux and preserves prior
LSM results and checks for other attributes. It has no target registry or
per-command policy lifecycle. Conglobate never grants command CAP_SYS_ADMIN.
Read-only views, inode/descriptor confinement, capabilities and syscall filters
remain the boundary protecting supervisor resources from label mutation.

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
| Labeling boundary | Exact-name behavior, both assigned roots, missing-policy admission failure, read-only denial, unrelated attribute checks, no supervisor inode/descriptor exposure |
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

The main conglobate process owns image storage, command containment and
publication. A trusted setup helper establishes each command's isolation and
starts its untrusted payload. Namespace init reaps command descendants. No second
recovery supervisor or serialized resume protocol is needed. This does not remove
bounded cancellation, normal error cleanup, subreaping where needed, or explicit
fallible teardown. Destructors remain best effort, never completion evidence.

The step owner retains mapped namespace and storage lifetimes until commands
stop and descendants are drained. On abrupt supervisor death, the remaining
lifetime arrangement must remain safe until the environment stops the VM.
VM shutdown is not instantaneous and does not justify exposing privileged
handles or reusing storage while commands still execute. If cleanup cannot
finish while conglobate lives, fail and require disposal; never advance or publish.

Validation must include actual supervisor termination and externally triggered VM
shutdown, with no successful artifact accepted. Test the deployed environment's
contract rather than assuming a signal handler or destructor will enforce it.
Independent VMs provide build concurrency; persistent shared-host execution is
not covered. Preserved guardian/anchor experiments remain useful reference data,
but do not impose an implementation requirement.

## Historical dependency and validation evidence

The previous architecture's dependency investigations and milestone appendices
are preserved verbatim with checksums in
[preservation/registration-free-architecture-c1073b9/ARCHITECTURE.md](preservation/registration-free-architecture-c1073b9/ARCHITECTURE.md).
They describe their recorded revisions, not additional current requirements.
Current dependency pins are authoritative in Cargo.toml/Cargo.lock; milestone
acceptance and subsequent reopened gates are recorded in REIMPLEMENTATION.md.
