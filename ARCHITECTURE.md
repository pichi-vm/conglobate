# Replacement architecture

Milestone 2 architecture candidate. This specifies the new implementation, not the reference code.
DESIGN.md and SANDBOX-DESIGN.txt remain normative. Changes to their trust model
require a user decision. Compile-time restrictions complement kernel checks;
they do not prove that external state cannot change or that destructors run.

## Ownership and modules

### Linear workflow simplification

[`BUILD-FLOW.sh`](BUILD-FLOW.sh) is the explanatory sequence. The environment
prepares Git, networking, labeling policy and VM shutdown. `Pipeline` owns the
build workspace, one execution runtime and the sole publication point. Stages
borrow that runtime. A pass-through build needs neither workspace tmpfs nor
command runtime. Verified root identities do not require live mappings: assemble
the output chain only for carapace directives or implicit firmware tools. An
explicit firmware tool image does not require mapping an otherwise unchanged
output base. Finalization uses verified/sealed root identities directly.

`Pipeline::execute` now sequences bootstrap, carapace and firmware. Bootstrap
returns a sealed scute, formatter receipt and process-reaping owner; carapace
returns its extended chain; firmware returns captured PMI/DTB files. None
orchestrates another stage. `StageOutput` holds these results without command or
mount authority. The runtime closes before `StageOutput::assemble` transforms
the DTB, validates firmware compatibility, assembles the image and constructs
inventory. Pass-through uses the same finalization without execution resources.
Tool inputs remain distinct from output ancestry. There is no generic workflow engine
or persisted build state.

Carapace produces one retained scute for the entire nonempty stage. All directives
share its writable snapshot, as bootstrap and firmware share their disposable
writable roots across directives. Command mounts and isolation are recreated per
step. Completed command cleanup precedes sealing; runtime cleanup precedes
publication. Failure to establish quiescence rejects the build and leaves VM
disposal to the environment.

Finish this complete structure before another broad qualification round.
Focused compilation checks during refactoring do not advance adherence gates.

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

After descendant drain, release process-domain and namespace resources,
then unmount/finalize storage. There is no activation, disable, policy-close,
installation-retry or policy-identity-release state. Cgroup joining remains a
distinct protocol represented by `Joined`, including its acknowledgment and
descriptor closure. Errors continue to retain
resource owners, and unfinished steps cannot yield publication authority.

The following table maps the architecture to current source owners. Proposed
names from the original design are not additional implementation requirements.

| Module | Owned or immutable domain types | Boundary |
| --- | --- | --- |
| request, path | BuildRequest, ContextPath, ContextDirectory | Parse CLI and relative locators without granting filesystem authority |
| context, input | CapturedContext, ContextFile, CapturedBuild, ResolvedBuild | Exact committed objects, immutable captured bytes and verified input resolution; no command access |
| recipe, value | Recipe, Directive, FilesystemSpec, CommandTimeout | Raw deserialization followed by validated immutable values |
| plan | BuildPlan, LockedInput, InputRole, StepId | Resolve inheritance/references and requirements before command admission |
| cache, image | ResolvedImage, VerifiedChain, VerifiedScute, ImageReference, IndexDigest | Verified component identities and cache-backed chain inputs |
| storage | Workspace, Scratch, Volume<S>, BootstrapVolumes<S>, VolumeView | Tmpfs, verified cache-backed chain, device mapping and writable storage ownership |
| filesystem, formatter | Mount<S>, Ext4Format, CapturedFormatter, FormatterRuntime, FormatReceipt | Idmapped views and trusted, measured formatting with explicit cleanup |
| step, domain, process | Step<S, R>, StepCompletion<R>, CommandDomain, Supervision<S> | Retain authority through admission, cancellation, reaping and descendant drain |
| execution, isolation | HelperProgram, HelperInvocation, UserNamespace<Mapped>, CommandReceiver, AssignedRoots, LabelProbe | Trusted helper entry, private mounts, mapped identity and restricted labeling admission |
| network | NetworkInput, NetworkNamespace, CommandNetwork, CommandNetworking, ResolverInput, ResolverMount | Disabled default; captured external namespace handle and confined temporary resolver injection; environment owns NAT |
| copy | CopySources, CopySource, CopyDestination | Explicit selected context files; exclusive confined writes; no context mount |
| pipeline | Pipeline, BootstrapBuild, Bootstrapped, DerivedCarapace, FirmwareTools, StageOutput, CompletedImage | Sequence stages, retain completed outputs and reject publication on failure |
| firmware | DtbContract | Validate declared firmware contract; pipeline firmware owner extracts the declared pair after writers stop |
| recipe::transform, transform | TransformSelection, TransformOperation, PropertyValue, TransformedDtb | Checked DTB operations and explicit default selection |
| inventory | Inventory | Versioned deterministic encoding of actual completed work; decoding alone proves no execution or authenticity |
| output | StagedDirectory, AssembledImage, PublicationFailure | Staging, component assembly and atomic no-replacement publication |

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
  Quiescent, and infallible `finish` returns
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
  owner only after runtime cleanup and release of workspace-backed outputs into
  the staged artifact. Captured context is immutable input data, not a mounted
  checkout requiring teardown. Raw staging publication by itself
  is not a successful-build receipt.

No public Clone on authority owners. Read-only descriptions and content IDs may
be cloned. Public handles must not permit recovering unrestricted writable FDs. Private
fields and crate-private descriptor access enforce that API boundary; internal
trusted code still requires authority and lifetime review.
Borrowed root views cannot outlive their owner or overlap consuming unmount.
A process holding a descriptor is not represented by a Rust borrow: Step must
retain actual kernel/process leases until quiescence is independently established.

Consider invariant generative build/step brands only if ownership aggregates do
not adequately prevent cross-build mixing. Ordinary lifetime parameters alone
do not establish instance identity. Defer that complexity until an API experiment
shows a concrete benefit. Thread-affine setup guards must not be Send/Sync merely
because their fields happen to be; namespace mutation remains isolated to an
appropriate single-threaded helper. Immutable plans may be freely shared.

## Error and cleanup ownership

Every fallible consuming transition describes the external state on failure.
Return the original typed owner only if its invariant remains known true.
Otherwise return a dedicated failure owner retaining remaining resources and primary plus
cleanup diagnostics. Do not claim retryable mounted/unmounted state after an
uncertain partial operation. No successful output token is available on failure. Quiescent means no remaining
command authority, not command success. Step<Quiescent> retains the drained
helper; its later completion carries the trusted helper status and storage.
The recipe command outcome is separate: CommandResult preserves CommandOutcome
through cleanup, and the execution owner carries it through finalization.
Successful helper exit or cleanup cannot substitute for successful command
execution. The enclosing execution owner checks that outcome and clean storage
finalization before permitting the pipeline to continue.
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
3. Release command namespace ownership after cgroup removal and process drain.
   The environment labeling exception remains installed until VM shutdown.
4. Remove temporary resolver/runtime attachments and release borrowed network handles, preserving image state and the environment-owned network.
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

BuildPlan is immutable. Stage owners retain verified inputs and produce an
AssembledImage plus execution data for inventory; references to inherited blobs
need not copy their bytes into workspace storage. A coordinator sequences stage methods without owning
all low-level setup logic or mutating a shared manifest throughout the build.

BootstrapBuild coordinates storage owners for a disposable tool layer and a new writable output layer
over dm-zero. Trusted formatting happens through the assigned output device.
Each command gets / and /mnt views; restricted setup validates labeling on both
using the environment exception. Mount and descriptor confinement isolate them
from supervisor resources. COPY uses the same root layout. No rootfs file import/conversion.
Successful teardown seals the output layer and discards tool changes.

DerivedCarapace derives the stage output layer while preserving every inherited component.
FirmwareTools owns one disposable tool root, with no special output /mnt. It
extracts declared paths only after quiescence and returns captured files. The
shared output finalizer applies transforms and checks firmware compatibility.
TransformSelection operates without a command sandbox. Missing vs empty sections,
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

The rustix Git revision `8bda151df87574f56f1e4c0b59f2d1df5d61609e`
provides the safe `mount_setattr` interface with a lifetime-bound `MountAttr`.
The locked registry rustix 1.1.5 does not expose this interface. Conglobate uses
it for idmapped filesystem views and restricted mounts of the resolver,
formatter runtime and command runtime. This pin keeps application code free of
unsafe syscall wrappers; it is not a claim that every upstream change has been
audited. Cargo.lock fixes the exact source identity, and deny.toml explicitly
allows that Git source. A separate registry rustix remains through tempfile.
Dependency policy checks cover both architecture graphs; runtime mount behavior
requires the separate disposable-VM qualification evidence.

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
| Network | Disabled default, externally supplied build namespace/NAT, explicit resolver including confined symlinks and cleanup |
| Outputs | Paired firmware validation, final DTB, deterministic CBOR, inherited blobs, OCI/ORAS, atomic failure cases |
| Stage-level layering | Multiple run/COPY steps share one writable carapace snapshot; exactly one new scute after success, none for an empty stage, none publishable after failure |
| Platforms | amd64/arm64 and TDX/SEV-SNP/CCA evidence; unavailable environments remain outstanding |

### Current requirements-to-tests mapping

These are navigation anchors, not blanket passing claims. Source tests establish
only their asserted property. Current VM evidence applies to native amd64 on the
recorded ordinary VM kernel. Historical fixtures referencing removed policy APIs
must be migrated before they count as current lifecycle regression coverage.

| Contract | Source/test anchors | Evidence boundary or remaining work |
| --- | --- | --- |
| Committed local inputs, explicit paths and locked indexes | tests/contexts.rs, requests.rs, locks.rs, cache.rs; src/input.rs | Rootless parsing/object/cache checks; commands must still be checked for context exclusion |
| Stage combinations, inheritance and variables | tests/plans.rs, recipes.rs, templates.rs; src/plan.rs | Planning assertions do not prove every privileged combination executes |
| Sealed states and borrowed mount authority | src/storage/volume.rs, filesystem.rs, step.rs compile-fail examples; src/step/execution/storage.rs | Compile-time ownership plus trusted storage-derived handoff; partial-acquisition failures require VM coverage |
| Format schema, geometry and inventory receipts | src/recipe/mod.rs, formatter/profile.rs, inventory/schema.rs; tests/distro/bootstrap-labeling/verify.py | Current minimal bootstrap verifies graph/receipt; exhaustive formatter failures remain to migrate |
| Restricted labeling admission | src/isolation/labels.rs; tests/distro/labeling/verify_missing.py, labeling-fault/verify.py | Current missing-policy and differential second-root negatives; test-only fault policy is not production registration |
| Persisted foreign labels and attribute boundaries | tests/distro/label-operations | Current set/read/remove/remount and exact-name negatives; target enforcing boot requires distro qualification |
| Command isolation and descendants | tests/distro/confinement, descendants | Current representative FD/capability/path/namespace observations and FIFO child cleanup; not exhaustive syscall/platform coverage |
| Error and timeout cleanup | tests/distro/failures; src/isolation/gate.rs tests | Current nonzero/timeout and gate boundaries; receiver start-send failure, helper/supervisor death, blocked I/O and writeback fault coverage remain |
| Confined COPY and firmware extraction | src/copy/destination.rs tests; tests/copy_modes.rs, copy_patterns.rs; src/pipeline/firmware.rs | Rootless path/metadata semantics; composed current-lifecycle execution coverage remains to migrate |
| DTB values, regex and firmware measurement compatibility | src/transform.rs, firmware.rs tests; tests/templates.rs | Pure transform/contract assertions; actual firmware generation and boot remain separate |
| External network inputs, DNS and resolver restoration | src/network; tests/lifecycle/src/network.rs; tests/distro/network | Current namespace-input, DNS/restoration and wrong-type VM evidence in preservation/m7-review/external-network; ingress/IPv6 and full failure qualification remain outstanding |
| Inventory and atomic self-contained publication | src/inventory.rs, output.rs, output/assembly.rs tests; tests/distro/bootstrap-labeling/verify.py | Current minimal emitted graph/canonical inventory; ORAS and full combination/failure reruns remain required |
| Sequential pipeline and one scute per carapace stage | src/pipeline.rs, pipeline/output.rs, pipeline/carapace.rs; tests/distro/pipeline/verify.py | 0556f30 full CLI composition checks two produced scutes, alternate firmware tools and final DTB root binding; empty-stage/failure combinations remain outstanding |
| Distro/platform qualification and preservation | tests/distro/{fedora,debian}, boot.py; REIMPLEMENTATION.md | Historical distro runs do not qualify changed lifecycle; Alpine/NixOS, arm64/confidential hardware and durable external-artifact recovery remain outstanding |

## Dedicated-VM lifecycle

Conglobate is the sole build workflow in a disposable VM. An independent
execution-environment controller shuts down the VM if conglobate crashes or exits
unexpectedly; the binary has no pichi/VM transport dependency. The controller must
not launch another build or permit abandoned resources to be reused in that VM.
This contract replaces the separate guardian and cross-invocation recovery design.
Control mounts are tied to the supervisor's acquisition mount namespace, not
PID 1's namespace. Cleanup verifies that the current mount namespace still
matches the pinned namespace descriptor. The environment owns crash shutdown;
the binary does not monitor init to establish cleanup authority.

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

## External-network implementation boundary

The environment, optionally using systemd services, provisions the enabled build
network and owns its namespace lifetime, veth/interfaces, addressing, routes, NAT
and ingress policy. Conglobate receives --network-namespace and --resolv-conf.
It captures an owned namespace descriptor once, validates nsfs and distinct identity,
and retains it through command quiescence; the path is not reopened per step.
The namespace belongs to the initial user namespace and differs from the
supervisor network namespace. Image code cannot configure it or receive its FD.
A supplied namespace is not proof of NAT or firewall policy: the trusted
environment must satisfy that contract. Namespace setup requires safe library
interfaces; application unsafe code remains forbidden.

Enabled steps reuse that build-dedicated namespace. Disabled steps receive fresh
disconnected namespaces. Resolver injection and restoration remain conglobate's
responsibility. Borrowed descriptor release does not destroy or reconfigure the
external network. No network namespace creator, allocation lease, address/route
selection, NAT installation, or network teardown retry owner is needed for the
enabled path. Resolver-mount cleanup and command descendant ownership remain.

The old network implementation and tests are preserved as reference at e249a90
and preservation/m7-review/external-network/; they are not evidence of the
replacement boundary. Maintained tests cover input validation and pinned handles
in tests/lifecycle/src/network.rs, with command DNS/restoration, wrong-type
rejection and environment retention in tests/distro/network/. The trusted
disposable-VM fixture provisions NAT. Broader authority/failure qualification
remains subject to the milestone gates. Keep external namespaces out of output artifacts;
network step permissions remain recorded in inventory.

Capture validates nsfs and distinct identity. A bind-mounted namespace's procfs
FD link may name its mount path rather than its type. Explicit CLONE_NEWNET at
setns enforces namespace type in trusted setup before command admission.
Initial-user-namespace ownership is an environment
requirement, not a new ioctl preflight. The network handle predates every command
user namespace and cannot be owned by one of those later-created namespaces.
CommandPrivileges excludes NET_ADMIN, NET_RAW and SYS_ADMIN from its allowed
capabilities. No command can acquire namespace administration through this input.
The trusted environment still must not supply concurrent services or mutate the
network in ways that violate ingress/reachability requirements.

Captured networking is one immutable aggregate of a namespace handle and resolver
bytes; its absence represents a plan with no enabled steps. An enabled plan
requires both CLI inputs. Per-step CommandNetwork retains a duplicated handle and
ResolverSource; only resolver source cleanup is fallible. Failure retains that
source for bounded retry. Remove network-creator/NAT recovery variants from the
step coordinator; ordinary setup error rejection retains the blocked command
owner. Preserve the current descriptor-transfer protocol and join-before-user-
namespace ordering; receiver changes must not expose a network FD to commands.
