# Conglobate image build design

Status: implemented standalone-binary contract. Requirements use “must”
or direct statements of behavior. The final sandbox contract and ordered
implementation outline are in [SANDBOX-DESIGN.txt](SANDBOX-DESIGN.txt).
Deferred ecosystem work is collected in [Remaining decisions](#13-remaining-decisions).
QUALIFICATION.md records the demonstrated environments and remaining platform limits.

[BUILD-FLOW.sh](BUILD-FLOW.sh) sketches the linear control flow and
resource ownership. It is explanatory shell notation, not executable sandbox code
or a substitute for the implementation and its tests.

Conglobate builds images from committed source and immutable image inputs. It
can create a new carapace using a tool carapace, extend an existing image, build
replacement firmware, and transform the final device tree. It emits an
importable image and an inventory binding its supplied inputs to its outputs.

## Trust model — required for attested builds

**The sandbox must work inside confidential VMs on Intel, AMD, and Arm.**
Hardware qualification is deferred by user decision; ordinary-VM evidence must
not be represented as confidential-platform qualification. Backend selection
must preserve command isolation and image metadata semantics on all three
platforms. See [the preserved platform review](ARCHIVE.md)
for candidate constraints and the distinction between source analysis and
hardware validation.

**Build commands are untrusted. The build kernel, supervisor, environment-provided BPF LSM
labeling exception and formatting tools are trusted.** Running without host root does not
by itself establish this boundary.

The builder verifies inputs, interprets recipes, assembles storage, formats and
mounts filesystems, enforces execution policy, finalizes outputs, and generates
the inventory. Kernel filesystem drivers replace the separately sandboxed FUSE
worker. Commands run in mapped user namespaces with private mount/PID/IPC/UTS/
network namespaces. Idmapped mount views preserve target ownership on disk.
The kernel parses filesystem contents: this increases the shared-kernel attack
surface compared with an isolated userspace worker. Content verification does
not prove that filesystem metadata is safe to parse.

Commands may modify only their assigned working filesystems and explicitly
authorized outputs, and use explicitly provided runtime resources. They must
not access or interfere with builder processes, backing devices, namespace
handles, control interfaces, inventory, output staging, or immutable inputs.
The build context is not mounted into commands; COPY exposes selected files.
Untrusted filesystem paths, symlinks, and metadata must not redirect trusted
builder operations outside those authorized effects.

Isolation must cover process access, inherited descriptors, mount reachability,
IPC, and network interfaces, not merely hide paths or process names. Sharing
a UID or using namespace root is not by itself evidence of isolation.
The builder must retain an independent way to stop commands and abort failed
filesystem service. Resource exhaustion may fail a build; it must not cause
an incomplete or compromised build to be accepted.

Before sealing an output, the builder must terminate and reap all command
descendants, revoke their write access, and finish filesystem writeback and clean unmount.
The bytes hashed for the inventory must remain stable through publication.
Future attestation must bind the trusted execution environment, including the
kernel, environment-provided labeling policy, formatter identities and effective configuration, to that inventory; measuring conglobate alone is
insufficient.

This is the agreed target model, not a claim about the existing executor.
The kernel-mount/LSM prototype demonstrates selected ext4 compatibility and
isolation properties. Full bootstrap integration and platform qualification
remain required; historical FUSE experiments are not the current backend.

The sections follow a build from input selection through execution and output
verification. Read the architecture and input sections first, then the pipeline
and recipe reference. Resource ownership, measurement, and the inventory define
the implementation's trust boundaries.

## Terminology

| Term | Meaning in this design |
| --- | --- |
| Image | A manifest and its content-addressed components: a carapace, a firmware pair, or both, with associated configuration and metadata |
| Scute | One immutable filesystem block layer stored as a copy-on-write (COW) object and protected by dm-verity |
| Carapace | An ordered chain of scutes assembled into a read-only block device; its top root hash binds the chain |
| Tool carapace | A carapace supplying the filesystem in which a stage's commands execute; changes to it are disposable unless the stage explicitly exports them |
| Base image | The image inherited by the output, including its components beyond the carapace |
| PMI | Portable Machine Image, the boot-payload format consumed by the VM launcher |
| DTB | Device Tree Blob, a binary tree of nodes and byte-valued properties describing the platform and its boot configuration |
| Firmware pair | The PMI and detached DTB carried by an image; the firmware stage replaces them together |
| Corium | The guest boot/runtime component whose `corium=<root>` command-line argument selects the root carapace |
| Arma | The tool commonly used by firmware recipes to produce a PMI and detached DTB |
| Context | The committed project subtree containing recipes and source files |
| Inventory | A machine-readable statement identifying the supplied inputs, effective build configuration, and final outputs |

The OCI manifest digest identifies the image description. A carapace root
identifies its verified filesystem chain. Neither replaces the other: inherited
firmware is covered by the image description, while filesystem assembly uses
the carapace root.

## Contents

- [Architecture and scope](#1-architecture-and-scope)
- [Standalone CLI](#2-standalone-cli)
- [Git context capture](#3-git-context-capture)
- [Image locks](#4-image-locks)
- [Context and pipeline](#5-context-and-pipeline)
- [Execution recipes](#6-execution-recipes)
- [Build variables](#7-build-variables)
- [DTB transformations](#8-dtb-transformations)
- [Privileged execution and resource ownership](#9-privileged-execution-and-resource-ownership)
- [Firmware measurement boundary](#10-firmware-measurement-boundary)
- [Artifact and inventory](#11-artifact-and-inventory)
- [Validation and implementation sequence](#12-validation-and-implementation-sequence)
- [Remaining decisions](#13-remaining-decisions)

## 1. Architecture and scope

### 1.1 Component responsibilities

1. **Conglobate** is a standalone Linux binary. Given a Git commit, a context
   subdirectory, and a read-only image cache, it produces an
   importable artifact. The supervisor runs as root for storage assembly. Commands never
   receive that initial-namespace root authority in the target design.
   Conglobate has no code dependency on VM management, virtiofs, GPT
   transport, PID 1, or a particular hypervisor. Its supported deployment is the
   dedicated disposable VM with independent shutdown specified in section 9.
2. **The official build appliance** wraps that same binary in a bootable pichi
   image. Its wrapper initializes the guest, exposes the supplied inputs,
   invokes conglobate, exports results and optional attestation evidence, and
   shuts down. Transport and boot lifecycle belong to the wrapper.
3. **`pichi build`** implements the appliance contract. It prepares committed
   inputs and cached images, supplies resources, launches the appliance, and
   retrieves/imports the result. It does not interpret private intermediate
   files to reconstruct the output image itself.

Rootless input, planning and pure-operation tests run without VM infrastructure.
Privileged lifecycle tests run in disposable VMs under the independent shutdown
contract. Appliance boot tests verify the wrapper and transport separately.

A build has the following lifecycle:

1. Select a committed context and verify its locked image inputs.
2. Materialize the context so execution reads the same bytes that were verified.
3. Create or select the output carapace and execute its layer directives.
4. Inherit firmware or produce a replacement pair, then transform the DTB.
5. Package the final components and emit the input/output inventory.
6. In the appliance, optionally bind that inventory to attestation evidence.

The artifact and inventory are the standalone builder's outputs. Importing or
publishing the image, transporting it out of a VM, and obtaining hardware
attestation are surrounding integrations.

### 1.2 Implementation constraints

Conglobate uses `#![forbid(unsafe_code)]`. Privileged operations are implemented
through safe library APIs. This is a prohibition on unsafe code in conglobate;
it does not claim that every transitive dependency has no unsafe internals.

Use the ecosystem's storage, carapace, device-mapper, loop-device, and DTB
libraries where appropriate. Library selection and API adaptation remain
implementation work. Do not recreate low-level syscall interfaces or shell out
to system tools for the built-in DTB operations. Recipe-authored commands can
of course invoke tools inside their execution environment.

YAML is used for human-authored formats. CBOR is used for machine-generated
formats controlled by this project. Existing external formats, including OCI
JSON and Git objects, retain their native encodings.

### 1.3 Measurability and reproducibility

The builder must identify and consume stable supplied inputs. Git commits,
image manifest digests, and carapace roots provide different identities and
must not be conflated. A repository location or mutable image tag is not an
immutable identity.

Per-step network access is supported. Recording the supplied inputs and the
fact that networking was enabled does not identify every downloaded byte or
promise reproducibility. Network dependency pinning and reproducible OS/package
selection are outside the initial builder's guarantees. The inventory must not
claim a closed set of all influences when commands can acquire external input.

## 2. Standalone CLI

Required logical inputs are a local Git repository path, a full commit identity,
a subdirectory within that commit, context-relative recipe and lock paths,
and a read-only image cache. CLI:

```sh
conglobate build \
  --repository <local-path> \
  --commit <full-object-id> \
  --context <relative-subdirectory> \
  --recipe <context-relative-yaml-path> \
  --lock <context-relative-lock-path> \
  --cache <read-only-image-cache> \
  --output <directory>
```

Defaults and failure behavior:

- `--context` defaults to `.`. It cannot escape the checkout.
- `--recipe` and `--lock` are required, with no default filenames or discovery.
  Both paths are relative to the selected context, not the current directory or
  each other's parent. Resolve them through the existing committed-context path
  rules, including confined symlink handling. Absolute paths, escapes, missing
  files and non-file targets fail. Both inputs must come from the selected commit.
  The inventory records their selected paths and exact content identities.
- `--commit` identifies a commit, not a branch, tag, abbreviated hash, or index
  snapshot. Locator and identity are separate inputs.
- The cache is never mutated. Missing locked objects are errors; image tags
  are never resolved opportunistically during a build. Conglobate downloads
  no additional image artifacts.
- `--uid-map PARENT_START:COUNT` and `--gid-map PARENT_START:COUNT` are
  required together for executing stages (bootstrap, nonempty carapace, firmware).
  Each maps image IDs starting at zero onto a contiguous parent range. The
  environment reserves these ranges, excluding its service accounts and other
  workloads; conglobate does not allocate IDs or read subordinate-ID databases.
  Parent ID zero, empty ranges, overflow and the invalid all-ones ID are rejected.
  UID and GID counts may differ. All stages use the same supplied pair. Images
  using IDs outside those ranges are unsupported; no silent ownership rewriting.
  Pass-through builds do not need mappings and omit them from the inventory.
- `--network-namespace PATH` is required when any run step enables networking.
  Open and retain the namespace descriptor once before execution; do not reopen
  the pathname per step. It must be a network namespace distinct from the
  supervisor's. The environment supplies a namespace owned by the initial user
  namespace; conglobate does not inspect that ownership with an ioctl. It captures
  the handle before creating any command user namespace, and command capabilities
  exclude network administration. The environment owns
  its lifetime, interfaces, addresses, routes, NAT and firewall policy. Conglobate
  joins it only in trusted command setup and never exposes the handle to image
  commands. Sequential enabled steps reuse this namespace; it is dedicated to
  this build, with no unrelated processes or concurrent builds sharing it.
- `--resolv-conf PATH` is required when any run step enables networking. Capture
  its contents once and inject them temporarily, read-only, following symlinks
  confined to the command root. No implicit resolver or DNS relay. Resolver
  contents and their digest are omitted from inventory.
- `--formatter-runtime PATH` is required for bootstrap. This trusted-environment
  YAML input selects a captured formatter runtime; it is not a recipe property.
  It has exactly `root` (directory), `executable` (relative file path), and `files`
  (unique relative runtime file paths including the executable). A relative root
  is resolved against the manifest's parent. For example:

  ```yaml
  root: /
  executable: usr/bin/mke2fs
  files:
    - usr/bin/mke2fs
    # The environment also lists its loader and all required runtime libraries.
  ```

  The example's single-file list works only for a self-contained executable;
  dynamically linked formatters require their loader and libraries. Capture
  resolves file symlinks within `root`, rejects missing/nonregular files, and
  installs the builder-owned mke2fs configuration. The executable receives only
  the typed filesystem profile's arguments. No ambient runtime fallback or
  dependency downloads occur. Inventory records executed file identities and
  effective configuration/arguments, rather than trusting manifest locators as
  identities. The environment is responsible for supplying a trusted executable
  and complete runtime; untrusted recipe authors cannot override this selection.
- Successful output is published only when all components and the inventory
  are complete. Refuse an existing output destination rather than overwrite it.
- Logs go to stderr; the process exit status distinguishes success from
  failure. A precise exit-code taxonomy is deferred.
- No successful artifact is published for a failed build. Failure diagnostics
  may be retained separately and are not a successful build inventory.

Repository input is always local. Fetching belongs to the caller, not
conglobate. The initial implementation builds only for the host architecture;
there is no architecture override or implied emulation support. Resource
limits belong to the execution environment. The exit-code taxonomy remains
an implementation-interface detail.

## 3. Git context capture

Conglobate consumes a specified commit from a local repository, not a working
tree or index snapshot. Caller-side working-tree cleanliness policy is outside
this interface. Conglobate does not create repositories, commit files, or fetch
remote content.

The environment prepares the local Git repository and any working checkout.
Conglobate verifies the selected committed objects and captures their contents
privately. Recipes, lock files and COPY sources read those immutable captured
bytes directly; there is no second filesystem checkout or context mount.
Subsequent source changes cannot alter the captured bytes. The context is never
exposed to image commands. Submodules and Git LFS are initially rejected.

### 3.1 Context ownership

Conglobate does not clone, fetch or run checkout operations. It records the
repository object format, selected commit and context subtree identity in the
inventory. Input capture verifies object content against these identities;
filesystem paths and the caller's working tree do not substitute for verification.
Preserve committed bytes, executable bits and symlink targets. Do not execute
caller-provided hooks or checkout filters. Existing local repository/commit and
context-relative recipe/lock arguments remain the input interface.

Git's object format must be recorded. Whether SHA-1 Git identities require an
additional independently authenticated commitment for the intended threat
model remains a security-design question. Hashing received bytes alone does
not establish the author's intended identity.

## 4. Image locks

The committed YAML file selected by `--lock` maps each recipe image reference to one immutable OCI index digest. Proposed
wire shape:

```yaml
version: 1
images:
  registry.example.com/fedora:43:
    index: sha256:...
  registry.example.com/kernel-builder:43:
    index: sha256:...
```

Every locked reference must identify a multi-architecture OCI index. A direct
manifest is not accepted. One lock supports multiple architectures, but each
build selects only the host architecture.

Index selection uses the ecosystem's `platform.os: pichi` selector, with OCI
architecture names `amd64` or `arm64`, and the pichi artifact type on the index
descriptor. This identifies a VM artifact rather
than a container for the builder's Linux host; it makes no assumption about
the firmware's guest operating system. Output index entries use the same
selector.

The index transitively pins its per-architecture manifests and their blobs.
Do not duplicate child manifest digests or carapace roots in the lock.
Conglobate verifies the index from the read-only cache, selects the matching
platform manifest, verifies it, and obtains the carapace root from that
manifest. It verifies content and chain consistency against those identities.
The inventory records both the pinned index and the selected manifest.

Missing pins, missing cached content, incompatible or ambiguous platform
selection, and mismatches are errors. Mutable cache tags are never consulted
to override a pin. An image without a carapace cannot supply an execution
carapace. A build never rewrites the lock; lock creation and cache population
belong to the caller.

## 5. Context and pipeline

The context has no required configuration directory or filenames. For example:

```text
<committed-context>/
  recipes/image.yaml
  locks/images.lock
  config/
  sources/
```

Select those files with `--recipe recipes/image.yaml --lock locks/images.lock`.
These names are examples, not defaults. COPY sources remain relative to the
context root, even when the recipe lives in a subdirectory.

The recipe is one YAML mapping with optional `bootstrap`, `carapace`, `firmware`
and `transform` sections. `platform` remains reserved and unsupported initially.
Execution order is fixed, regardless of YAML key order:

```text
bootstrap -> carapace -> firmware -> [platform, deferred] -> transform
```

The lock is a separate input, not a stage. Reject duplicate or unknown keys and
unsupported sections rather than merging or ignoring them. Execution sections
are mappings; `transform` is a sequence. Use explicit `{}` or `[]` for empty
sections; YAML null is not an alias for omission or emptiness. An empty recipe
cannot produce an image.

For example, an inherited image with an explicitly disabled default transform:

```yaml
carapace:
  from: registry.example.com/fedora:43
transform: []
```

The pipeline carries an assembled image forward. Separately, a stage may use
a carapace as its execution environment. **A tool input does not become output
ancestry merely because commands execute inside it.**

### 5.1 Stage summary

| Stage | Execution input | Retained output |
| --- | --- | --- |
| Bootstrap | Required tool carapace | A new zero-backed carapace layer, formatted and populated directly |
| Carapace | Bootstrap output, otherwise required `from` image | Inherited image with additional scutes |
| Firmware | Resulting carapace, or explicit tool carapace | Replacement PMI and DTB together |
| Platform | Tool carapace, details deferred | Transformed DTB |
| Transform | No execution carapace | Declaratively edited DTB |

Omitted stages pass existing output forward. If bootstrap is present and
carapace is omitted, firmware can use the bootstrap result directly. A carapace
recipe with no directives is also a valid pass-through when an input exists.
Without bootstrap, that recipe still needs `from`; `carapace: {}`
cannot select an input image without bootstrap output.

All components of a base image propagate unless a stage replaces them.
There is no feature for explicitly discarding inherited firmware to obtain a
carapace-only derivative. A bootstrap tool image's components are not inherited
because it is not the base of the output image.

### 5.2 Output kinds and section combinations

Valid image kinds are carapace-only, carapace plus PMI/DTB, and PMI/DTB-only.
The last kind lives entirely within its firmware/initramfs environment.

| Bootstrap | Carapace | Firmware | Result before DTB transformations |
| --- | --- | --- | --- |
| Absent | Absent | Absent | Invalid: no image-producing stage |
| Present | Absent | Absent | New base carapace only |
| Absent | Present | Absent | Derived image, including all inherited components |
| Absent | Absent | Present | PMI/DTB-only; tool `from` required |
| Present | Present | Absent | New carapace plus layers |
| Present | Absent | Present | New carapace and new PMI/DTB |
| Absent | Present | Present | Derived carapace and replacement PMI/DTB |
| Present | Present | Present | New carapace plus layers and new PMI/DTB |

For an existing image requiring only DTB edits, a `carapace` section with `from`
and no directives selects and passes through the base image.

Existing launch configuration and other image components propagate with the
base. How a recipe creates or overrides those components is not yet specified;
the pipeline described here defines carapace and firmware production only.

## 6. Execution recipes

Examples in sections 6.1–6.4 show the contents of the named top-level section;
place them under that key in the single recipe file. Transform examples show
sequence contents unless explicitly wrapped in `transform`.

The shared vocabulary includes `from` and `derive`; firmware export uses `into`.
Bootstrap declares its output volume through `filesystem`. A `derive` list contains
ordered `run` and `copy` directives. Unknown fields and unsupported combinations
are errors rather than ignored instructions.

Commands execute as UID/GID 0 in a mapped user namespace, with a disposable
or layered working filesystem exposed through an idmapped kernel mount.
An environment-provided BPF LSM exception permits foreign SELinux labels without command
`CAP_SYS_ADMIN`; that capability is absent from every command capability set.
They never receive initial-user-namespace root authority.
Tools may be installed during execution; a tool carapace only needs enough
initial functionality to obtain and run them. A Fedora image with `dnf` can
build a new Fedora image containing `dnf`, which can then serve as a future
tool carapace. The first tool carapace is bootstrapped externally.

```yaml
derive:
  - run: dnf install -y kernel-devel
    network: true
    timeout: 5m
  - copy:
      from: ./source
      into: /build/source
```

Network access is opt-in per `run` directive and defaults to false. For enabled
steps conglobate joins the supplied `--network-namespace`; for disabled steps it
creates a fresh disconnected namespace. Commands cannot administer either.
The environment provides outbound NAT, permits reply traffic and all reachable
destinations, and prevents unsolicited inbound connections and inbound port
forwarding. Conglobate does not create interfaces, allocate addresses, inspect or
modify routes, install NAT/firewall rules, or require particular forwarding
sysctls in its own namespace. This is an environment contract, not a policy that
conglobate can infer from a namespace descriptor. No systemd dependency is imposed.
Enabled steps may share network state (including conntrack), but no command
process may survive its step. The namespace must not expose trusted controls.

Every `run` has a finite positive command-execution timeout, defaulting to `5m`.
The optional `timeout` field accepts duration strings such as `30m` or `1h`;
zero, infinite, invalid, and overflowing values fail validation. Setup and
teardown have separate fixed internal deadlines, not recipe or CLI settings.
Fresh filesystem mounts and a fresh command sandbox are used for each run step.

Common execution behavior:

- The committed context remains private to the builder. Commands receive
  selected contents only through explicit `copy` directives.
- Commands run through `/bin/sh -eu -c`, starting at `/`. Environment and
  working-directory changes are expressed within a command rather than
  persisted implicitly between shells. Processes cannot outlive a step.
- `copy.from` refers to context-relative paths. Copy behavior, including
  symlinks, follows Dockerfile `COPY` semantics. The implementation must use
  the Docker reference and compatibility tests rather than choose a blanket
  follow/preserve policy. This does not add unrelated Docker features such as
  multi-stage Dockerfiles or `ADD` URL fetching.
- Owner/group/mode handling follows the corresponding Docker `COPY` behavior
  for the fields conglobate supports. Metadata and SELinux behavior must be
  tested in the privileged execution environment.
- No uncontrolled inherited host environment is exposed to build commands.

`copy.from` accepts one string or a nonempty list of strings. `copy.into` is
an absolute image path; multiple sources require a trailing slash. Optional
`owner` and `group` accept quoted numeric IDs or names resolved in the image's
`/etc/passwd` and `/etc/group`. Defaults are UID/GID zero; specifying only a
numeric owner uses that ID for both. A named owner uses its primary GID from
`/etc/passwd` unless `group` explicitly overrides it. Optional `mode` is a quoted
octal or symbolic mode. Filename patterns use Go `filepath.Match` semantics, including
Unicode `?` and bracket classes, without shell brace expansion.

### 6.1 Bootstrap

`bootstrap` section creates a new carapace directly, using a separate tool carapace.
The supervisor creates a writable snapshot layer over dm-zero and formats its
block-device view with a trusted formatter. Formatting and all subsequent output
writes land in that layer. There is no intermediate rootfs image, image import,
or image-to-scute conversion.

The command sandbox has two assigned writable mounts:

- `/`: the tool carapace with a disposable writable layer. Authors may install
  additional tools here; those changes persist across bootstrap steps only.
- `/mnt`: the new output filesystem. Commands populate it using ordinary
  filesystem/package-manager operations, without block-device or mount access.

The initial filesystem declaration is:

```yaml
from: registry.example.com/fedora-tools:43
filesystem:
  type: ext4
  size: 4GiB
  properties:
    label: rootfs
derive:
  - run: dnf install -y dnf-plugins-core
    network: true
  - run: dnf --installroot=/mnt --releasever=43 install -y filesystem bash dnf
    network: true
```

`from` is required. `filesystem` declares filesystem capacity, filesystem type and supported typed
formatting properties. Initially `type` is `ext4`; `size` is a positive integer
followed immediately by `B`, `KiB`, `MiB` or `GiB`, must be 4096-byte aligned and
between 16 MiB and 64 GiB inclusive. No fractional values or implicit units.
`properties.label` defaults to empty and is limited to 16 UTF-8 bytes without NUL.
Unknown properties fail. The carapace logical device remains 64 GiB; trusted
formatting uses exactly the requested number of 4096-byte blocks, leaving the
rest zero-backed. Size is not permission to change the carapace wire geometry. Validate these before privileged
execution; do not accept arbitrary formatter arguments, executable paths or shell
fragments. Fail if the driver, trusted formatter or required idmapping/metadata
support is unavailable. Ext4 is the initially validated backend; other kernel
filesystems require qualification. Record the effective formatting configuration
and formatter identity with an explicit inventory schema revision.

`/mnt` is reserved for the output mount during bootstrap. Resolve its mountpoint
within the pinned tool root; never follow an image symlink outside that root.
COPY destinations use this same layout, so `/mnt/...` addresses output and other
paths address the tool root. Context is never mounted into commands. The trusted
environment provides the fixed exact-`security.selinux` labeling exception.
After restrictions are applied, validate labeling on both assigned roots before
admitting each command. Filesystem reachability, read-only mounts, descriptor
confinement and process isolation protect unrelated supervisor inodes; the
labeling policy does not maintain a per-filesystem authorization registry.

After writers stop and both filesystems are cleanly unmounted, seal/hash the
output scute and package its carapace. Discard the tool writable layer. No tool
scutes or firmware components are inherited; deliberate file copies can still
contribute output bytes. Bootstrap has no image-file `into` export. The output
becomes the implicit input of the `carapace` section, or passes forward when it is omitted.
The dual-mount snapshot-chain integration and both-root label admission are
covered by the retained pipeline and lifecycle evidence; see QUALIFICATION.md.

### 6.2 Carapace

`carapace` section retains filesystem modifications as new layers:

```yaml
from: registry.example.com/fedora:43
derive:
  - run: dnf install -y nginx
    network: true
  - copy:
      from: ./nginx.conf
      into: /etc/nginx/nginx.conf
```

When bootstrap supplies the input, `from` is forbidden. Otherwise it is
required. Selecting a base image inherits all its components. The existing
scutes pass through and new scutes chain onto the preceding root.

The retained model is one new scute per nonempty carapace stage. All `run` and
`copy` directives modify the same writable snapshot of the input carapace.
Each command retains separate isolation, timeout and network selection; stop
and reap its descendants, flush and unmount before proceeding to the next step.
Seal the shared snapshot only after every directive succeeds. A failed stage
produces no publishable layer. No directives means no new scute; skipping empty
block deltas is not part of the initial model.

This does not provide Docker-style intermediate layer caching. Files created
and deleted within the stage are not retained as separate historical layers,
but filesystem metadata and freed blocks may still contribute to its block
delta. Sealing is not filesystem compaction.

### 6.3 Firmware

`firmware` section replaces the PMI and DTB as a pair. It commonly invokes arma,
but is not restricted to Linux firmware. A custom firmware or UEFI-oriented
recipe can produce the supported output artifacts through its own tools.

Omitting `from` uses the resulting carapace. An explicit `from` selects a
different tool carapace, even when a result carapace exists. If there is no
result carapace, `from` is required. The execution input never determines which
scutes become output: a firmware-only build remains firmware-only.

```yaml
from: registry.example.com/kernel-builder:43
derive:
  - copy:
      from: ./kernel.config
      into: /build/linux/.config
  - run: |
      cd /build/linux
      make -j"$(nproc)"
      make modules_install
  - run: |
      mkdir -p /out
      dracut --force --no-compress /out/initramfs.cpio \
        "$(make -s -C /build/linux kernelrelease)"
  - run: |
      arma build \
        --kernel /build/linux/arch/x86/boot/bzImage \
        --config /build/linux/.config \
        --initrd /out/initramfs.cpio \
        --cmdline "console=hvc0 corium=$CARAPACE" \
        --dtb /out/base.dtb \
        /out/boot.pmi
into:
  pmi: /out/boot.pmi
  dtb: /out/base.dtb
```

This is an x86 Linux example. Its tool carapace must supply kernel sources,
compiler, arma, corium, and appropriately configured initramfs tooling. A
firmware-only recipe omits the carapace argument and includes its workload in
the initramfs. The example is a recipe shape, not a universal distro build.

All directives share a disposable writable tool root at `/`. Firmware has no
special `/mnt` output mount. `into` selects explicit paths inside the tool root;
extract both files through confined handles after all command writers stop.
Intermediate changes
do not become scutes. Only the declared PMI and DTB survive. When the stage is
absent, existing firmware propagates unchanged to DTB finalization.

### 6.4 Platform

`platform` section is reserved for arbitrary DTB transformations executed in a
tool carapace, with the same recipe vocabulary. Implementation and the exact
working-DTB interface are deferred until there is a user requiring it.

An implementation that does not support this stage must reject its presence,
not silently ignore it. Declarative transforms do not require this stage.

## 7. Build variables

After carapace construction, `CARAPACE` identifies the final output carapace
root as plain hex. It also exists when bootstrap produced that carapace and
`carapace` section was omitted.

- Shell commands access `$CARAPACE` or `${CARAPACE}` through the environment.
- Transformation replacements access `${CARAPACE}` through template expansion.
- A tool carapace's root must never be substituted for the output root.
- With no output carapace, the variable is undefined. An evaluated reference
  fails instead of expanding to an empty value.

`/bin/sh -eu -c` enables unset-variable checking and shell error-exit behavior.
Shell quoting and explicit shell constructs still have their normal meaning;
this is not static validation of every textual variable occurrence.

In transformation templates, uppercase names are reserved for build variables;
numbered or lowercase names refer to regex captures. `$$` writes a literal
`$`, so `$${CARAPACE}` writes the literal text `${CARAPACE}`. Expansion is
single-pass and never invokes a shell.

Build variables expand in `set` data of type `string` and `strings`, using the
same dollar escaping. Set values have no regex captures. Numeric and binary
set values remain explicitly typed data.

Regex replacement variables are evaluated only when a match is expanded. Zero
matches therefore do not evaluate `CARAPACE`. An evaluated undefined build
variable is an error in either set data or a regex replacement.

## 8. DTB transformations

The DTB is the canonical boot configuration artifact. In particular, Linux's
kernel command line, when present, is the string `/chosen/bootargs`. There is
no second persistent command-line definition outside the DTB.

`transform` section is an ordered list of operations executed directly through
safe Rust libraries. Each item has exactly one `op` discriminator. `set` also
has a flat `type` discriminator selecting the encoding of `data`.
`property` names an absolute node path followed by the property name; for
example, `/chosen/bootargs` selects `bootargs` on the `/chosen` node.

Operations can have an `if` condition:

```yaml
- op: regex
  if:
    exists: /chosen/bootargs
  property: /chosen/bootargs
  match: '(^|\s)corium=[^\s]*'
  replace: '${1}corium=${CARAPACE}'
```

`if.exists` requires a DTB. It returns false when the named node or property
is absent and skips the operation before reading its value. An attempted read
of a missing property still fails. Existence does not validate type: a present
non-string property still fails a regex operation.

The following illustrates all operations. It assumes an input DTB containing
`/some-device/obsolete-option`, because removing a missing property fails.

```yaml
- op: set
  property: /chosen/bootargs
  type: string
  data: "console=ttyS0,115200 quiet"

- op: set
  property: /compatible
  type: strings
  data: ["example,my-machine", "example,my-platform"]

- op: set
  property: /some-device/clock-frequency
  type: u32
  data: 24000000

- op: set
  property: /memory@80000000/reg
  type: cells
  data: [0x0, 0x80000000, 0x0, 0x40000000]

- op: set
  property: /some-device/enabled
  type: empty

- op: set
  property: /some-device/vendor-data
  type: bytes
  data: "01 ab 00 ff"

- op: remove
  property: /some-device/obsolete-option

- op: regex
  property: /chosen/bootargs
  match: '(^|\s)console=ttyS0,115200'
  replace: '${1}console=hvc0'

- op: regex
  property: /chosen/bootargs
  match: '$'
  replace: ' systemd.show_status=false'
```

The illustrative `enabled` property has no universal device-tree meaning;
bindings determine the meaning of properties. The editor controls encoding,
not device-specific policy. Parent-cell layouts, phandle consistency, and
firmware hardware constraints remain the author's responsibility unless
explicitly validated by the relevant downstream consumer.

### 8.1 Set and remove

`set` creates or replaces the property, creating missing ancestor nodes in an
existing DTB. A missing DTB is an error; `set` does not implicitly invent a
machine description. `remove` deletes an existing property and fails when its
node or property is absent.

| Type | Encoding |
| --- | --- |
| `string` | One UTF-8 string with a trailing NUL |
| `strings` | Ordered sequence of NUL-terminated UTF-8 strings |
| `u32` | One big-endian unsigned 32-bit integer |
| `u64` | One big-endian unsigned 64-bit integer |
| `cells` | Ordered big-endian unsigned 32-bit integers |
| `empty` | Present property with zero bytes; no `data` |
| `bytes` | Literal byte sequence expressed as hexadecimal |

Reject invalid widths, out-of-range values, malformed hex, and embedded NULs
in individual strings. YAML uses strict booleans and does not interpret leading
zero integers as legacy octal. Integer data accepts YAML integer notation
within its unsigned width. Hexadecimal byte data ignores ASCII whitespace,
accepts either letter case, and requires an even number of hex digits with no
`0x` prefix. Arbitrary property bytes are
representable without assuming that a DTB contains runtime type tags.

### 8.2 Regex

`regex` operates on exactly one NUL-terminated UTF-8 string, excluding its
terminator. A string list, binary property, malformed string, absent property,
or absent DTB is an error for an authored operation.

**Replace every match; zero matches leave the string unchanged.** There are
no match-count settings. Target validation happens before matching: an invalid
string is an error even if the expression would have no matches.
Invalid regex syntax or invalid replacement-template syntax fails validation.

| Target state | Authored regex operation |
| --- | --- |
| DTB, node, or property missing | Fail |
| Property is not a single valid string | Fail |
| Valid string, zero matches | Succeed without changing it |
| Valid string, one or more matches | Replace all matches in that operation |

Use Rust `regex` syntax and capture expansion, with the build-variable extension
described above. Do not implement sed syntax or shell out. `${1}` in the
examples preserves the whitespace consumed by the boundary capture.

### 8.3 Default selection

- Absent `transform` section: select the built-in sequence.
- Present `transform` section: use exactly the authored sequence.
- `transform: []`: perform no transformations (a DTB is still required).
- There is no inclusion or automatic appending of defaults to authored edits.

The built-in sequence is:

```yaml
- op: regex
  if:
    exists: /chosen/bootargs
  property: /chosen/bootargs
  match: '(^|\s)corium=[^\s]*'
  replace: '${1}corium=${CARAPACE}'
```

It refreshes an existing `corium=` argument and never introduces one. Firmware
authors establish the initial binding. A matched binding with undefined
`CARAPACE` fails. Non-string command-line data remains an error. This generic
regex does not validate all corium arguments or repair duplicate bindings.

When no DTB exists, skip only the implicit default transformation stage.
An authored `transform` section requires a DTB, including an explicitly empty
sequence. Within an existing DTB, the default uses `if.exists` to skip an absent
command line. No operation reads a missing property successfully, and a
missing DTB never makes an existence condition return false.

The inventory records the effective default and whether its stage or operation
was skipped. Property operations only are in scope initially; `set` may create
ancestor nodes, but there are no explicit whole-node creation/removal operations.

## 9. Privileged execution and resource ownership

[SANDBOX-DESIGN.txt](SANDBOX-DESIGN.txt) is the authoritative sandbox contract.
It specifies the ordered setup, execution, teardown, finalization, and validation
requirements. This section summarizes its relationship to the build pipeline.

Conglobate runs as the only major workflow in a dedicated disposable build VM.
The execution environment must independently shut down that VM if conglobate
crashes or exits unexpectedly. It must not start another build or reuse abandoned
kernel resources in that VM. This is an environment contract, not a pichi library
or transport dependency. The binary does not implement VM lifecycle management.
A separate in-guest guardian and cross-invocation recovery are not required.
Normal step failures and bounded teardown remain conglobate's responsibility.
An incomplete cleanup is terminal: return an error, publish nothing and require
VM disposal. No recovery/reuse API or retry of a failed build is required.
Abrupt supervisor death also invalidates the build and is handled by VM disposal.

The root supervisor owns verified inputs, cache handles, loop/device-mapper
assembly, writable snapshots, inventory, and publication. Immutable carapace
layers remain cache-backed with verified reads. One private build-wide tmpfs
holds mutable snapshots and scratch; ordinary tmpfs behavior applies. The
execution environment configures capacity and resource limits, with no disk
fallback or per-step memory policy. Final artifact storage is separate.

The supervisor mounts assigned kernel filesystems and creates idmapped views
for commands. Superblocks remain supervisor-owned; commands receive namespace
root semantics through mapping, without backing-device access. No userspace
filesystem worker is required. Ext4 is initially validated. Other drivers,
formatters and metadata combinations require explicit qualification.

The trusted environment loads and retains a fixed BPF LSM exception for exactly
`security.selinux`. Conglobate does not compile/load/attach the policy or register
subjects and filesystems. The VM has SELinux disabled; foreign labels belong to
the target OS and confer no authority in the build environment. Before recipe
admission, verify required behavior under restricted command credentials; missing
support fails without a capability fallback. See SANDBOX-DESIGN.txt section 4.

Label mutation is not constrained by ordinary file write-mode checks. Integrity
therefore depends on inode/descriptor reachability and read-only views, alongside
normal filesystem and remaining LSM checks. Never expose mutable supervisor,
cache, context or control inodes to commands. Both bootstrap roots use the same
fixed exception without a target registration protocol.

Each run remounts its assigned filesystems and launches a fresh command sandbox.
COPY and extraction occur only while command writers are stopped, using scoped
handles and root-confined paths. Commands have no `CAP_SYS_ADMIN` in any
capability set, restricted syscalls/ioctls, `no_new_privs`, private proc/dev/devpts/
shm, empty read-only sysfs, and explicit environment and stdio. Namespace/mount
creation and control access are prohibited. Context enters only through COPY.

Commands never share the supervisor network namespace. Enabled steps join the
environment-owned build namespace with outbound NAT without destination filtering; supervisor controls must not be
exposed on reachable network addresses. The environment supplies `--resolv-conf`;
there is no built-in DNS relay, resolver inventory entry or persistent injection.

On every exit path, stop/reap all descendants before removing runtime mounts
and resolver mounts, flushing and cleanly unmounting filesystems for sealing.
Release borrowed network handles without dismantling the environment-owned network. Retain
namespace handles and storage leases as needed for their actual resource
lifetimes, with no policy-specific identity pins or revocation transition. If
quiescence cannot be proved, retain ownership and reject publication. Recover
partial setup while conglobate is alive. If safe teardown cannot complete, fail
the build and require VM disposal; do not start another step or reuse resources.
The environment-owned labeling exception remains loaded until VM shutdown.
Propagate filesystem/writeback errors; forced/lazy unmount is not evidence of
clean finalization. Supervisor death is an environment shutdown test, not an
in-guest recovery protocol.

Foreign target SELinux labels, ownership, ACL semantics, and file capabilities
must survive into the booted image independently of the build VM policy. The
validated foreign-label environment disables build-VM SELinux; an enforcing
configuration requires separate qualification. Initially unsupported are
`trusted.*` access, persistent device creation, and immutable/append-only flag
changes (including clearing inherited flags). Named pipes (FIFOs) are supported
filesystem objects; permitting FIFO mode does not permit character or block
device creation. Document failures explicitly.

This contract requires containment of untrusted commands. It is not a claim
that the legacy executor already meets it or that prototype checks prove
absence of sandbox vulnerabilities.

## 10. Firmware measurement boundary

Firmware production precedes final DTB transformations. The final artifact
and inventory must identify the transformed DTB, and any measurement or
signature commitment covering it must use those final bytes.

Detached does not automatically mean freely mutable: the relevant PMI/firmware
contract determines which modifications are valid. Firmware formats that bind
the DTB before transformation require a compatible final sealing step or must
reject such transformations. This ordering must be reconciled with arma and
PMI contracts before promising support for arbitrary firmware signing flows.

The default corium edit packages an existing root-binding convention. Generic
DTB editing does not prove that custom firmware consumes the root securely.

The initial implementation accepts changed DTBs only for recognized PMI VM,
SEV, TDX, and CCA target manifests with exactly one external `dt:dtb` fill
action and sufficient reserved section capacity. Every target in the PMI must
accept the edit. A pre-signed SEV identity, bundled-only DTB, unknown target,
or opaque firmware rejects a changed DTB. Unchanged DTBs require no resealing
and pass through, including opaque custom firmware. Authors using unsupported
signing contracts must generate the final DTB during firmware production and
disable subsequent transforms with `transform: []`.

## 11. Artifact and inventory

The output directory is itself a self-contained OCI image layout, with the
successful build inventory alongside its standard entrypoints:

```text
result/
  oci-layout         # OCI layout version; external OCI formats remain JSON
  index.json         # output image index
  blobs/sha256/      # all referenced manifest, configuration and layer blobs
  inventory.cbor     # successful input/output binding
```

OCI consumers use `result/` directly. Attestation and any additional evidence
binding the inventory bytes belong to the appliance, as specified in section 11.3.

Include every blob referenced by the output image, including inherited blobs
from the cache. Import must not require the original cache. The destination
must not already exist, even if empty. Packaging validates structure and
content identities before publishing the completed output directory.
The final manifest describes all components and the final transformed DTB.

### 11.1 Inventory schema

`inventory.cbor` is versioned machine data. The following is a YAML rendering
of its proposed logical shape, not a second emitted inventory format or a
finalized CBOR wire schema:

```yaml
version: 3
builder:
  name: conglobate
  version: ...
  executable: {algorithm: sha256, digest: <bytes>}
context:
  repository: <local-path>
  object_format: sha1
  commit: <bytes>
  subdirectory: project
  tree: <bytes>
inputs:
  - roles: [carapace-base]
    reference: registry.example.com/fedora:43
    index: {algorithm: sha256, digest: <bytes>}
    manifest: {algorithm: sha256, digest: <bytes>}
    carapace_root: <bytes>
  - roles: [firmware-tools]
    reference: registry.example.com/kernel-builder:43
    index: {algorithm: sha256, digest: <bytes>}
    manifest: {algorithm: sha256, digest: <bytes>}
    carapace_root: <bytes>
execution:
  id_mappings:         # effective mappings; omitted for pass-through
    uid: {parent_start: 200000, count: 131072}
    gid: {parent_start: 400000, count: 131072}
  architecture: amd64
  recipe:
    path: recipes/image.yaml
    content: {algorithm: sha256, digest: <bytes>}
  lock:
    path: locks/images.lock
    content: {algorithm: sha256, digest: <bytes>}
  transform:
    source: builtin
    digest: {algorithm: sha256, digest: <bytes>}
    target_present: true
  formatters: []        # nonempty for bootstrap; records described below
  network_steps:
    - {stage: carapace, step: 0}
outputs:
  manifest: {algorithm: sha256, digest: <bytes>}
  carapace_root: <bytes>
  scutes:
    - digest: {algorithm: sha256, digest: <bytes>}
      origin: inherited
    - digest: {algorithm: sha256, digest: <bytes>}
      origin: produced
  pmi: {algorithm: sha256, digest: <bytes>}
  dtb: {algorithm: sha256, digest: <bytes>}
```

Digest payloads are CBOR byte strings with explicit algorithm identifiers.
Absent output components are omitted. Recipe and lock paths are relative to
the selected context. Their digests identify the exact committed file bytes,
not a YAML reserialization. Network steps identify the stage section and its
zero-based directive index. Input roles refer to use, not output ancestry. Source locators
are provenance hints, not authenticated identities, and must not expose
embedded credentials.

Resolver configuration and its digest are deliberately excluded from inventory;
network-enabled steps are still recorded. This environment configuration is not
claimed to be an exhaustively measured input. Inventory coverage otherwise must
include effective build options, selected architecture,
the committed lock, all supplied image manifests, recipe contents, and the
effective built-in transform version/content when applicable. Some identities
are transitively covered by the context or image manifest; explicit entries
make auditing useful without inventing independent trust anchors. The manifest
identity binds all output components, including inherited configuration not
listed separately in the example. The inventory must not imply that only
scutes, PMI, and DTB are inherited.

Inventory encoding is deterministic CBOR. The implementation must specify the
exact deterministic encoding profile, required fields, unknown-field/version
handling, ordering of arrays, and digest algorithms.
Do not bind evidence to a human-readable reserialization. Bind the exact
emitted CBOR bytes. Deterministic serialization does not imply a deterministic
build or require timestamps in every inventory.

Formatter receipts identify each executed backend profile, effective filesystem
specification (type, capacity in bytes, label and logical device capacity), exact
executable digest, captured runtime-file digests with paths, configuration digest
and effective argument vector. Dynamic device paths are locators, not identities.
The `formatters` array is empty when no formatting ran. Receipt creation follows
successful execution/finalization; planned work alone is not evidence of execution.
Each formatter receipt has exactly these fields: `stage` (initially `bootstrap`),
`profile` (versioned backend name), `profile_digest` (Digest), `executable`
(`path` and `digest`), `runtime` (path/digest records sorted by unique path),
`configuration` (Digest), `arguments` (ordered strings), and `filesystem`
(`type`, `size_bytes`, `logical_size_bytes`, `label`). All are required; Digest
uses the algorithm/byte-string representation above. Arrays preserve execution
order. No ad-hoc extension to version one is allowed.

### 11.2 Inventory wire profile and schema migration

The deterministic encoding profile uses RFC 8949 section 4.2.1 core
deterministic CBOR:
shortest integer and length encodings, definite-length containers, and map keys
ordered by the bytewise lexicographic order of their deterministic encodings.
Floating-point values and CBOR tags are not part of this schema. Digests and
Git object identities are byte strings. SHA-256 digests have 32 bytes; Git
identities have 20 or 32 bytes according to `object_format`.

The required top-level fields are `version`, `builder`, `context`, `inputs`,
`execution`, and `outputs`, with the nested fields illustrated above. Version
is 2 for the target schema described here. Consumers reject unknown fields,
unsupported versions, duplicate keys, noncanonical encodings, and trailing data. Absent optional output roots
and firmware digests are omitted. An input's `carapace_root` is also omitted
when a firmware-only base passes through without execution; execution-tool
inputs must have a 32-byte root. Such a base retains the `carapace-base` role,
which identifies the selecting recipe, not a requirement that scutes exist.
`outputs.scutes` and `inputs` are arrays,
including when empty. Scutes are recorded in base-to-top chain order; inputs
are sorted by reference, with unique sorted roles. Network steps occur in
pipeline and directive order.

The single-recipe design replaces version one's `execution.recipes` map with
explicit `execution.recipe` and `execution.lock` path/content records, and
replaces per-file network-step identifiers with `stage`. This requires a schema
version change; the existing version-one implementation must not silently emit
these fields as version one. Version three additionally records effective environment-supplied UID/GID
mappings in execution.id_mappings. Emit and validate version three explicitly.
The transform record adds a
required boolean `default_skipped`, indicating that the implicit default
encountered no DTB or no bootargs property; an authored transform always records
false for this field. `target_present` records whether a DTB was supplied.
For `authored`, the transform digest is the SHA-256 of the exact committed
recipe bytes containing that section (also identified by `execution.recipe`).
This binds the section without inventing a YAML reserialization. For `builtin`,
it is the SHA-256 of the builder's exact built-in sequence bytes, including when
the default is skipped. Changes to those bytes change the recorded identity.
`source` is `builtin` or `authored`. Input roles are `bootstrap-tools`,
`carapace-base`, and `firmware-tools`; scute origins are `inherited` or
`produced`. The builder record hashes the running executable. Local repository
and context paths are provenance locators, not additional trust anchors.

Finalization checks inventory output identities against the finalized manifest
and validates every referenced blob before publication. Output is staged under
a temporary sibling directory and published using an atomic no-replace rename.
The resulting layout includes the canonical empty config blob when referenced;
replaced, unreferenced blobs are not exported.

### 11.3 Attestation

The guest generates the inventory from what it verified, consumed, and
produced. Evidence binds its digest to the measured execution environment.
The builder executable digest alone does not identify the kernel, libraries,
or other privileged environment that determined the result.

Standalone execution emits the same inventory without claiming attestation.
Initial conglobate emits inventory only. Attestation belongs to the appliance
integration; evidence remains separate
to avoid circular hashing. The report-data encoding, endorsements, verifier
policy, freshness, and hardware-specific mechanism are deferred.

Outputs must be hashed from the finalized bytes the trusted builder actually
produced, before relying on an untrusted output transport. Re-reading mutable
host output and treating it as authoritative would break that binding. A
consumer can verify retrieved bytes against the attested inventory.

An inventory records an execution claim. It does not establish that recipes
are benign, that network dependencies are reproducible, or that the author
intended the pinned inputs without a separate authenticity policy.

## 12. Qualification and retained evidence

The implementation follows the ordered sandbox contract. QUALIFICATION.md and
REIMPLEMENTATION.md record completion, three-pass adherence gates, actual test
environments and limits. Maintained tests cover committed inputs, locks, typed
DTB operations, pipeline combinations, metadata, deterministic inventory,
publication and disposable-VM lifecycle failures. Distro recipes exercise Fedora
DNF/SELinux, Debian APT/systemd, Alpine APK/OpenRC and NixOS closure construction.
A separate firmware recipe constructs an initramfs and arma PMI/DTB, then boots it.

CI checks native amd64/Arm64 Rust and guest lifecycle behavior. Arm64 hosted guest
tests explicitly record TCG when KVM is unavailable. This does not propose TCG as
the production sandbox. Confidential-computing hardware qualification is deferred
by user decision; appliance transport and attestation remain separate work.

Historical FUSE/UML/kernel-mount experiments, superseded implementations, original
reviews and raw failures/successes are recoverable through ARCHIVE.md. They are
historical evidence, not alternative active designs. The binary remains standalone
and does not create VMs; privileged tests run only inside disposable VMs.

## 13. Remaining decisions

The initial implementation uses gix and verifies each consumed Git object's
identity during private capture. Recipe symlinks resolve only to captured files
within the context; hooks and checkout filters do not execute. The lock and
inventory wire formats, YAML values, device exposure, and supported firmware
measurement contracts are specified above. Temporary writable mappings use
kernel deferred removal and scratch leases for in-process cleanup after failed
activation; abrupt conglobate death is handled by disposal of the build VM. Ordinary tmpfs allocation failures propagate as build failures.

Remaining ecosystem decisions are authentication of the intended Git identity,
creation/override of components beyond carapace and firmware, additional
firmware signing/resealing contracts, and appliance transport and attestation.

Sandbox policy is settled in SANDBOX-DESIGN.txt. Exact capability/syscall/ioctl
profiles, fixed internal deadline values, trusted formatter/property schemas,
both-root labeling admission, identity allocation, and runtime mount mechanics
are implemented under
that contract. Appliance integration and confidential-platform qualification remain separate;
fully unprivileged storage assembly is outside the selected design.

The initial scope excludes remote Git retrieval, uncommitted working-tree
inputs, submodules, Git LFS, cross-architecture builds, build-time lock updates,
output-cache mutation, firmware discard, partial PMI/DTB replacement,
transform-default inclusion, regex match-count options, whole-node operations,
and execution of `platform` section. Caller working-tree policy is outside this
interface.
