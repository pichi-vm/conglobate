# Implementation architecture

DESIGN.md and SANDBOX-DESIGN.txt define the contract. This document maps that
contract to the implementation. BUILD-FLOW.sh shows the same linear sequence;
it is explanatory notation, not an executable security boundary.

## Inputs and workflow

The environment supplies committed Git objects, a read-only local image cache,
reserved UID/GID ranges, optional network namespace and resolver file, trusted
formatter files, the fixed labeling exception and independent VM disposal.
Conglobate performs no artifact downloads, Git clone/checkout, network setup,
policy registration or attestation. It has no dependency on the pichi binary.

`BuildRequest`, `CapturedBuild`, `ResolvedBuild` and `BuildPlan` progressively
validate and capture inputs. Git object identities and the consumed context are
verified; COPY uses captured bytes without exposing a context mount. Locked
multi-architecture indexes select only the host architecture. Image components
and verified chain roots remain separate from live device mappings.

`Pipeline::execute` sequences bootstrap, carapace and firmware directly. It owns
one RAM workspace and execution runtime, borrowed by stages. Bootstrap formats
its new writable snapshot over dm-zero and uses separate tool/output mounts.
Carapace directives share one writable snapshot and produce one scute for the
whole stage. Firmware uses disposable tools and extracts the PMI/DTB pair.
Tool inputs contribute inventory provenance, not accidental output ancestry.

`StageOutput` holds results without command/mount authority. Runtime cleanup
precedes DTB transformation, image assembly, deterministic CBOR inventory and
publication. Pass-through builds use the same finalization without allocating
an execution runtime. Publication is a no-replacement atomic rename after all
fallible checks; no successful image is exposed after a failed build.

## Ownership and Rust states

Application Rust forbids unsafe code. Methods belong to types that own their
invariants. Private fields, sealed marker traits, immutable validated values and
consuming transitions express what each operation may do. Important examples:

- `Volume<Unmounted>` and `Volume<Mounted>` retain mapping/mount ownership.
  `VolumeView` borrows a root from storage actually retained by the step.
- `Step<Ready>`, `Step<Running>`, `Step<Cancelling>`, `Step<Reaping>`,
  `Step<Draining>` and `Step<Quiescent>` constrain command lifecycle operations.
- Formatter capture, staging, execution and completion own distinct resources.
- Failure values retain the resources required for explicit cleanup/retry and
  preserve both operation and cleanup diagnostics.

RAII provides ordinary scoped cleanup; destructors cannot prove quiescence or
successful writeback. Explicit fallible completion performs those checks.
Terminal failure retains diagnostic truth and requires VM disposal when cleanup
cannot finish. There is no guardian process or persisted recovery state machine.

## Storage

Immutable aligned scutes stay cache-backed. RAM-backed copies are used where
compression or padding requires materialization; writable layers and filesystem
workspaces are always RAM-backed. Cache identity checks and authenticated reads
remain distinct protections. Device-mapper and loop owners retain descriptor and
mount-namespace identity through partial setup, teardown and retry.

The carapace format uses a 64 GiB dm-zero origin. Bootstrap's configured ext4
capacity is a filesystem capacity within that geometry, not a format change.
The initial backend accepts aligned 16 MiB–64 GiB capacities and typed ext4
properties. Trusted mke2fs uses 4096-byte filesystem blocks. Finalization trims
unused snapshot capacity, uses ecosystem wire types and seals the scute directly;
there is no image-to-carapace import/conversion step.

Scute wire parsing belongs to devmap-snapshot. Conglobate owns activation and
fallible cleanup using devmap/lodown: a read-only file with deferred removal alone
cannot represent its partial-setup/cleanup failures. It does not duplicate the
wire codec. The pinned rustix revision supplies the safe borrowed-descriptor
mount_setattr API (upstream PR 1630); the pin is intentional, not a mutable branch.

## Command boundary

The supervisor and kernel are trusted. Commands receive mapped namespace-root
credentials, private mount/PID/user and other isolation, private proc/dev/devpts,
restricted capabilities and a native-ABI seccomp allowlist. They never receive
SYS_ADMIN, backing devices, the context, supervisor resources or host terminal
handles. Command stdout/stderr are pipes relayed by trusted code; stdin is null.
PTY queries/unlock/raw-mode settings and caller-local parent-death signals support
Nix builders. Terminal injection/control and namespace/mount creation stay denied.

The environment's fixed BPF LSM exception permits exactly security.selinux
labeling without making the target filesystem's policy the build VM's policy.
Conglobate probes labeling under the actual command restrictions before admission,
including both bootstrap roots. It neither loads nor revokes the exception.
Ordinary ownership, ACL, mount and filesystem checks still apply.

Each command reconstructs mounts/isolation and has a finite recipe timeout
(default five minutes). Setup and teardown use separate fixed internal deadlines.
Descriptor handshakes establish readiness; sleeps are not synchronization.
Cgroup containment, process identity, cancellation, reaping and descendant drain
precede releasing mounts or sealing storage. Independent environment shutdown
handles supervisor death; another build must never reuse that abandoned VM.

Networking is offline by default. Enabled commands join an environment-supplied,
validated and pinned namespace and receive captured read-only resolver bytes.
The environment owns outbound NAT, ingress policy, DNS and namespace lifetime.
Resolver mounts follow symlinks only within the command root and restore prior
image state. Conglobate implements no NAT allocator, resolver service or networkd
integration.

## Qualification and maintenance

See QUALIFICATION.md for actual source versions, environments, successes and
limits. Tests cover rootless contracts, disposable-VM lifecycle/failures, full
pipeline composition and real distro builds/boots. Native amd64/Arm64 Rust checks
and guest lifecycle cases run in CI. Confidential-computing qualification is
deferred; no attested-appliance or universal-kernel claim follows from these tests.

Historical implementations, reviews, experiments and their failures are preserved
under the verified archives described in ARCHIVE.md. Maintained regression tests
and the final contracts are the active repository's reference points.
