# Conglobate image build design

Status: design target for the locked build-VM redesign (docs-first).
Requirements use “must” or direct statements of behavior. The crate is a
types-only library plus a binary engine; the standalone CLI remains first-class
and VM-free for tests. The [sandbox lifecycle](#14-sandbox-lifecycle) and
[Rust architecture](#15-rust-architecture) are specified below.
Deferred ecosystem work is collected in [Remaining decisions](#13-remaining-decisions).
[Validation scope](#12-validation-scope) describes maintained coverage and platform limits.
Implementation of the full engine reshape is separate from this design lock-in.

Conglobate builds images from committed source and immutable image inputs. It
can create a new carapace using a tool carapace, extend an existing image, build
replacement firmware, and transform the final device tree. It emits an
importable image and an inventory binding its supplied inputs to its outputs.

## Trust model — required for attested builds

**The sandbox must work inside confidential VMs on Intel, AMD, and Arm.**
Hardware qualification is deferred by user decision; ordinary-VM evidence must
not be represented as confidential-platform qualification. Backend selection
must preserve command isolation and image metadata semantics on all three
platforms. Ordinary-VM tests do not establish confidential-hardware compatibility.

**Build commands are untrusted. The build kernel, supervisor, environment-provided BPF LSM
labeling exception and image-resident formatting tools (selected by the internal
filesystem-name → formatter map) are trusted.** Running without host root does not
by itself establish this boundary. Host-supplied formatter runtimes are not part
of the design.

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
kernel, environment-provided labeling policy, appliance image (and its internal
formatter profiles) and effective configuration, to that inventory; measuring
conglobate alone is insufficient.

The implementation uses kernel ext4 mounts and an environment-provided labeling
exception. The trusted environment implements that exception as a pure-Rust Aya
program and a Rust load, attach and pin path (section 14.4). Conglobate does
not compile or load it. Confidential-hardware qualification remains deferred;
tests do not prove absence of sandbox vulnerabilities.

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
  - [Build appliance](#14-build-appliance)
  - [Appliance build-port CBOR](#15-appliance-build-port-cbor)
- [Standalone CLI](#2-standalone-cli)
- [Git context](#3-git-context)
- [Image locks](#4-image-locks)
- [Context and pipeline](#5-context-and-pipeline)
- [Execution recipes](#6-execution-recipes)
- [Build variables](#7-build-variables)
- [DTB transformations](#8-dtb-transformations)
- [Privileged execution and resource ownership](#9-privileged-execution-and-resource-ownership)
- [Firmware measurement boundary](#10-firmware-measurement-boundary)
- [Artifact and inventory](#11-artifact-and-inventory)
- [Validation scope](#12-validation-scope)
- [Remaining decisions](#13-remaining-decisions)
- [Sandbox lifecycle](#14-sandbox-lifecycle)
- [Rust architecture](#15-rust-architecture)

## 1. Architecture and scope

### 1.1 Component responsibilities

1. **Conglobate crate** is a types-only `[lib]` plus a `[[bin]]` engine.
   The library exposes shared types (including OCI/image output types formerly
   owned by `pichi-artifact`); those types retire into this crate so OCI outputs
   are Conglobate outputs. The binary owns the build engine. **`pichi` depends
   on the Conglobate lib** for those types; it does not re-own artifact schemas.
2. **Standalone CLI** (`conglobate build`) remains a first-class, VM-free entry
   point for rootless checks and engine tests. Given a local Git repository, a
   commit (or resolved ref), a context subdirectory, recipe/lock paths, a
   read-only image cache and an output directory, it produces an importable
   artifact and inventory without requiring virtiofs, a hypervisor, or the
   appliance image. The supervisor runs as root for storage assembly when
   privileged stages run; commands never receive that initial-namespace root
   authority. The engine has no code dependency on VM management, virtiofs, GPT
   transport, or a particular hypervisor.
3. **Build appliance** is a pichi image **shipped from this repository**
   (bootstrap CI in the same class as `fedora`/`carapace` images). Self-host
   later via `pichi build`. The appliance is how production builds run; see
   [§1.4](#14-build-appliance).
4. **`pichi build`** is a specialization of `pichi run` (owned by pichi's docs).
   It prepares committed inputs and cached images, launches this appliance, and
   retrieves/imports the result. It must not contradict this appliance contract.
   It does not interpret private intermediate files to reconstruct the image.

Rootless input, planning and pure-operation tests run without VM infrastructure.
Privileged lifecycle tests run in disposable VMs under the independent shutdown
contract. Appliance boot/CI verifies the image and transport separately.

A build has the following lifecycle:

1. Select a committed context (resolve ref→commit when given a ref) and verify
   locked image inputs.
2. Load recipe and lock immediately from the object store; resolve other context
   reads lazily with verify-on-use (see [§3](#3-git-context)).
3. Create or select the output carapace and execute its layer directives.
4. Inherit firmware or produce a replacement pair, then transform the DTB.
5. Package the final components and emit the input/output inventory.
6. In the appliance, optionally bind that inventory to attestation evidence.

The artifact and inventory are Conglobate outputs. Importing or publishing the
image, transporting it out of a VM, and obtaining hardware attestation are
surrounding integrations.

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

### 1.4 Build appliance

Conglobate ships its build-appliance as a **pichi image from this repo**, built
by bootstrap CI (same class as the `fedora`/`carapace` images). Later, the
appliance may self-host via `pichi build`.

Bootstrap CI (`.github/workflows/build.yml`, sources under `image/`) builds
that image with mkosi from Fedora 43 packages. It does not layer the published
fedora carapace: that image does not treat `virtiofs` as first-class, so this
rootfs keeps `virtiofs` and `fuse` and loads them after the initramfs pivot.
systemd is PID 1 and its units bind tags `repo`, `cache`, and `output`.
`e2fsprogs` supplies image-resident `mke2fs`. The release binary is installed
at `/usr/bin/conglobate`. On `main`, CI pushes a multi-arch index to
`ghcr.io/pichi-vm/conglobate` tagged `:0.2.0` (this crate's version) and
`:latest`, with `platform.os` `pichi` and architectures `amd64` and `arm64`.
`:0.2.0` is the durable pin (plus the digest once pushed). Consumers must
not rely on `:latest` alone.
Pull requests build the image and do not push. `image/conglobate.recipe.yaml`
stays a self-host-later skeleton; it is not this bootstrap.

Product assumptions for the guest:

1. **PID 1 is systemd.** There is no Conglobate-as-`/init` product path and no
   custom mount generator.
2. **Static virtiofs mounts** (configured in the image/unit, not by Conglobate
   parsing mount tags from the host response) expose host tags:
   - `repo` → `/run/pichi/repo`
   - `cache` → `/run/pichi/cache`
   - `output` → `/run/pichi/output`
3. **Named build-port console** `dev.pichi.build`, opened read/write through
   the kernel console/hvc sysfs path (not a udev symlink). The channel is
   bidirectional (guest→host tx and host→guest rx).
4. **v0 wire (one round trip, self-delimiting CBOR, no length prefix; no
   streaming/multiplexing):** the guest opens the port, writes one request map
   `{"v":1,"op":"build-input"}`, then reads one host response map. The guest
   must **not** assume the host already pushed bytes before the request (this
   supersedes host one-shot CBOR push / uni-rx-only). On `ok:true`, Conglobate
   runs the engine against the mounted paths and publishes into the empty
   `/run/pichi/output` mount. The locked end state is the OCI layout
   (`oci-layout`, `index.json`, `blobs/`, `inventory.cbor`). Beside it, the
   engine dual-writes a transitional flat `build.yaml` (`BuildOutput` /
   `ScuteOut`) plus delta scute files `NNNN.cow` and `NNNN.verity` in chain
   order (base first) and, when this build produced firmware, `boot.pmi`.
   Inherited base scutes are not repeated; the host prepends the operator
   source carapace from `from:`. An empty delta list is valid here. OCI-only
   output is not enough for the current host smoke.
   On `ok:false`, it fails with the host `error` string.
   There is **no** local CBOR copy retained as a second source of truth, **no**
   Conglobate parse of virtiofs field readers / mount tags from the response,
   and **no** mount generator in this crate.
5. Appliance image hardcodes (not CBOR fields): UID/GID maps, the network
   namespace path used when networking is enabled, and the resolver file path.
   Formatting tools live in the image; see [§2](#2-standalone-cli).
6. **Guest exit.** After `conglobate appliance` exits, success or failure, the
   oneshot powers the VM off so the launcher is not left at a login prompt.
   Appliance stdout and stderr are mirrored to the kernel console
   (`console=hvc0`) and the journal.

### 1.5 Appliance build-port CBOR

Transport is the named console in [§1.4](#14-build-appliance). Maps are **flat**
(not a nested transport envelope). Self-delimiting CBOR; no length prefix.

**Guest → host request (exactly one map):**

| Field | Required | Meaning |
| --- | --- | --- |
| `v` | yes | Protocol version; must be `1` |
| `op` | yes | Must be `"build-input"` |

**Host → guest response (exactly one map):**

| Field | Required | Meaning |
| --- | --- | --- |
| `v` | yes | Protocol version; must be `1` |
| `ok` | yes | `true` on success; `false` on failure |
| `error` | when `ok` is `false` | Human-readable failure reason from the host |
| `ref` | when `ok` is `true` | Git ref name or full object id; Conglobate resolves it to a commit on the mounted repo |
| `recipe` | when `ok` is `true` | Path to the recipe YAML, always relative to `subdir` |
| `lock` | when `ok` is `true` | Path to the lock YAML, always relative to `subdir` |
| `subdir` | no (success) | Context subdirectory within the commit; default `"."` |
| `location` | no (success) | Optional provenance/locator hint for inventory; not an authenticated identity |

Explicit non-fields on the success response (must not appear; if present, reject):

- `commit` (guest resolves `ref` → commit)
- virtiofs mount tags or mount paths
- `output` destination (fixed at `/run/pichi/output`)
- formatter runtime / mke2fs host manifest
- UID/GID maps, network-namespace path, resolv-conf path
- `op` (request-only)

Recipe and lock paths are always relative to `subdir`. Conglobate opens the
mounted repository at `/run/pichi/repo`, resolves `ref` to a commit, and uses
`/run/pichi/cache` as the read-only image cache.

**Ref resolution (guest):** resolve `ref` with gix on the mounted repository to a
single commit object. Accept a full object id (hex matching `object_format`) or a
ref name; peel annotated tags to the commit. Reject abbreviated object ids,
missing refs, and non-commit tips. The resolved commit is the inventory identity;
optional `location`, when present, is recorded as inventory
`context.repository` (provenance locator); when absent, record the fixed mount
path `/run/pichi/repo`. Neither is an authenticated identity. The host `ref`
string itself is not a separate inventory field — only the resolved commit is.

**Wire decoding / failure rules (v0):**

- Maps use text-string keys, definite-length CBOR, and only the fields in the
  tables above. Reject duplicate keys, noncanonical encodings that the decoder
  does not normalize, unknown fields, and wrong types.
- Decode exactly one top-level response map; after that map, any leftover bytes
  already in the read buffer are trailing data and fail the build (same spirit
  as inventory trailing-data rejection).
- Require `v == 1` before interpreting other fields; version mismatch fails.
- When `ok` is `false`, only `v`, `ok`, and `error` may appear; success-only
  fields must not appear. When `ok` is `true`, `error` must not appear.
- Missing build-port device, undecodable CBOR, truncated response, or read
  timeout (fixed internal deadline) fails the build. There is no silent default
  recipe/lock/`ref`.

**Output mount:** `/run/pichi/output` is an empty writable virtiofs mount supplied
by the host (not a path Conglobate creates). Conglobate stages privately in a
temporary child directory on that mount, then publishes by moving each top-level
layout entry (`blobs/`, `oci-layout`, `index.json`, `inventory.cbor`,
`build.yaml`, each flat `NNNN.cow` / `NNNN.verity` / `boot.pmi`, and any
other exported roots) into the mount root with no-replace semantics (refuse if
any destination name already exists). Fail if the mount is missing, not a
directory, or non-empty before publication. This is distinct from CLI `--output`,
which names a new directory that must not already exist (see [§11.1](#111-inventory-schema)
finalization / publication and [§14.2](#142-resolve-and-verify-all-supplied-inputs-before-execution)).

## 2. Standalone CLI

The standalone CLI exercises the same engine without a VM. It is the path used
by rootless CI and local tests. It does not claim the appliance image's
formatter-tool binding or build-port attestation; those are appliance-only.
Appliance production builds use the CBOR
contract in [§1.5](#15-appliance-build-port-cbor) instead of these host flags for
request transport; both paths share recipe/lock/cache semantics.

Required logical inputs for the CLI are a local Git repository path, a full
commit identity (or a ref the CLI resolves locally), a subdirectory within that
commit, context-relative recipe and lock paths, a read-only image cache, and an
output directory:

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

- `--context` defaults to `.`. It cannot escape the checkout. (Appliance CBOR
  uses `subdir` for the same role.)
- `--recipe` and `--lock` are required, with no default filenames or discovery.
  Both paths are relative to the selected context, not the current directory or
  each other's parent. Resolve them through the committed-context path rules,
  including confined symlink handling. Absolute paths, escapes, missing files
  and non-file targets fail. Both inputs must come from the selected commit.
  The inventory records their selected paths and exact content identities.
- `--commit` identifies a commit, not a branch, tag, abbreviated hash, or index
  snapshot. Locator and identity are separate on the CLI. The appliance passes
  `ref` instead; the guest resolves `ref` → commit on the mounted repository.
- The cache is never mutated. Missing locked objects are errors; image tags
  are never resolved opportunistically during a build. Conglobate downloads
  no additional image artifacts.
- **Formatting is outside the command sandbox.** Bootstrap selects a formatter
  from an **internal filesystem-name → formatter map** in the engine/image.
  The appliance image installs `e2fsprogs`, so `mke2fs` resides at the standard
  paths that map probes first. Capture still reads that binary and its dynamic
  dependencies from the guest root; standalone CLI builds use the same probe
  on the host. There is **no** `--formatter-runtime` flag and **no**
  host-supplied mke2fs (or other formatter) manifest as a TEE input. Recipe
  authors cannot override formatter selection. Inventory records the effective
  internal formatter profile and filesystem specification; it does not treat a
  host path locator as a formatter identity.
- `--uid-map PARENT_START:COUNT` and `--gid-map PARENT_START:COUNT` remain
  available on the standalone CLI for VM-free and fixture execution of stages
  that need mappings. In the **appliance image** these ranges are hardcodes,
  not CBOR fields and not host CLI flags. Each maps image IDs starting at zero
  onto a contiguous parent range. Parent ID zero, empty ranges, overflow and
  the invalid all-ones ID are rejected. UID and GID counts may differ. All
  stages use the same pair. Images using IDs outside those ranges are
  unsupported; no silent ownership rewriting. Pass-through builds do not need
  mappings and omit them from the inventory.
- `--network-namespace PATH` and `--resolv-conf PATH` remain available on the
  standalone CLI when any run step enables networking (same capture/join rules
  as before). In the **appliance image** the namespace path and resolver file
  are hardcodes, not CBOR fields. Open and retain the namespace descriptor once
  before execution; do not reopen the pathname per step. Capture resolver
  contents once and inject them temporarily, read-only, following symlinks
  confined to the command root. No implicit resolver or DNS relay. Resolver
  contents and their digest are omitted from inventory.
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

## 3. Git context

Conglobate consumes a specified commit from a local repository (CLI) or from the
appliance-mounted repo after resolving `ref` → commit. It does **not** use a
working-tree checkout as source of truth. Caller-side working-tree cleanliness
policy is outside this interface. Conglobate does not create repositories,
commit files, or fetch remote content.

### 3.1 Immediate vs lazy reads

1. **Recipe and lock** are loaded **immediately** from the Git object store and
   verified against their object identities before pipeline execution.
2. **COPY and other context reads** use **lazy gix verify-on-use**: each path is
   resolved and content-verified when first needed. There is **no** eager full
   context capture into a private tree as the design SoT, and **no** second
   filesystem checkout or context mount for commands.
3. Verified bytes are immutable for the rest of the build. Subsequent source
   changes in a caller working tree cannot alter what Conglobate reads.
4. The context is never exposed to image commands; COPY exposes selected files.
5. Submodules and Git LFS are initially rejected.

### 3.2 Context ownership

Conglobate does not clone, fetch or run checkout operations. It records the
repository object format, selected commit (after ref resolution when applicable)
and context subtree identity in the inventory. Paths and the caller's working
tree do not substitute for object verification. Preserve committed bytes,
executable bits and symlink targets. Do not execute caller-provided hooks or
checkout filters.

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
block-device view **outside the command sandbox**, using the engine's internal
filesystem-name → formatter map (initially ext4). Formatting and all subsequent
output writes land in that layer. There is no host-supplied formatter runtime
manifest, no intermediate rootfs image, image import, or image-to-scute
conversion.

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
fragments from the recipe or host. Fail if the driver, internal formatter map
entry or required idmapping/metadata support is unavailable. Ext4 is the
initially validated backend; other kernel filesystems require qualification.
Record the effective formatting configuration and internal formatter profile
identity with an explicit inventory schema revision.

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
Maintained pipeline and lifecycle tests exercise dual-mount snapshot chains
and label admission on both roots.

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

[Section 14](#14-sandbox-lifecycle) specifies the detailed sandbox contract.
It specifies the ordered setup, execution, teardown, finalization, and validation
requirements. This section summarizes its relationship to the build pipeline.

Conglobate runs as the only major workflow in a dedicated disposable build VM.
The execution environment must independently shut down that VM if conglobate
crashes or exits unexpectedly. It must not start another build or reuse abandoned
kernel resources in that VM. This is an environment contract. The Conglobate engine does not implement VM
lifecycle management; the appliance image and `pichi build`/`run` own that.
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
`security.selinux`. That program, and its load, attach and pin path, are a
pure-Rust Aya rewrite owned by the environment (section 14.4). Conglobate does
not compile, load or attach the policy, does not depend on libbpf, and does not
register subjects and filesystems. Its only labeling role is `LabelProbe`
admission. The VM has SELinux disabled; foreign labels belong to the target OS
and confer no authority in the build environment. Before recipe admission,
verify required behavior under restricted command credentials; missing support
fails without a capability fallback. The C sources
`tests/environment/labels.bpf.c`, `tests/environment/loader.c` and related C
loaders such as `tests/distro/labeling-fault/loader.c` remain an interim
disposable-VM qualification harness until the Aya path exists, and are then
retired. See the labeling requirements in section 14.4.

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

This contract requires containment of untrusted commands. Passing compatibility
and isolation tests does not prove absence of sandbox vulnerabilities.

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
  build.yaml         # transitional BuildOutput; scutes may be empty
  0000.cow           # delta scutes only, chain order; names come from build.yaml
  0000.verity
  boot.pmi           # only when this build produced firmware
```

OCI consumers use `result/` directly. The transitional `build.yaml` and flat
blobs are for the current host packaging path: filenames are relative to
`result/`, chain order is base to top, and only delta scutes are listed. The
host prepends the operator source carapace named by `from:`. `inventory.cbor`
remains the successful input/output binding; that host smoke may ignore it.
Attestation and any additional evidence
binding the inventory bytes belong to the appliance, as specified in section 11.3.

Include every blob referenced by the output image, including inherited blobs
from the cache. Import must not require the original cache. Packaging validates
structure and content identities before publishing the completed layout.
CLI `--output` names a destination that must not already exist, even if empty;
the appliance `/run/pichi/output` mount is an exception — it pre-exists empty and
receives layout entries as specified in §1.5 and §11.2. The final manifest
describes all components and the final transformed DTB.

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
  repository: <provenance-locator>   # CLI: local path; appliance: location or /run/pichi/repo
  object_format: sha1
  commit: <bytes>                    # resolved commit identity; host ref tip not stored
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

Formatter receipts identify each executed **internal** backend profile, effective
filesystem specification (type, capacity in bytes, label and logical device
capacity), configuration digest and effective argument vector. They bind the
image-resident formatter profile, not a host TEE `--formatter-runtime` manifest.
Dynamic device paths are locators, not identities. The `formatters` array is
empty when no formatting ran. Receipt creation follows successful
execution/finalization; planned work alone is not evidence of execution.
Each formatter receipt has exactly these fields: `stage` (initially `bootstrap`),
`profile` (versioned backend name from the internal map), `profile_digest`
(Digest of the selected internal profile's effective definition as implemented
by the engine), `configuration` (Digest), `arguments` (ordered strings), and
`filesystem` (`type`, `size_bytes`, `logical_size_bytes`, `label`). All are
required; Digest uses the algorithm/byte-string representation above. Arrays
preserve execution order. Version three replaces version-one host-runtime
`executable` / `runtime` receipt fields with `profile` / `profile_digest`;
emit and validate that receipt shape only under inventory version three. Do not
emit or accept the version-one host-runtime fields as version three.

### 11.2 Inventory wire profile and schema migration

The deterministic encoding profile uses RFC 8949 section 4.2.1 core
deterministic CBOR:
shortest integer and length encodings, definite-length containers, and map keys
ordered by the bytewise lexicographic order of their deterministic encodings.
Floating-point values and CBOR tags are not part of this schema. Digests and
Git object identities are byte strings. SHA-256 digests have 32 bytes; Git
identities have 20 or 32 bytes according to `object_format`.

The required top-level fields are `version`, `builder`, `context`, `inputs`,
`execution`, and `outputs`, with the nested fields illustrated above. Version is 3 for the target schema described here (the example in §11.1). Consumers
reject unknown fields, unsupported versions, duplicate keys, noncanonical
encodings, and trailing data. Absent optional output roots and firmware digests
are omitted. An input's `carapace_root` is also omitted when a firmware-only base
passes through without execution; execution-tool inputs must have a 32-byte root.
Such a base retains the `carapace-base` role, which identifies the selecting
recipe, not a requirement that scutes exist. `outputs.scutes` and `inputs` are
arrays, including when empty. Scutes are recorded in base-to-top chain order;
inputs are sorted by reference, with unique sorted roles. Network steps occur in
pipeline and directive order.

Version three is the complete current inventory shape. Relative to version one it
replaces `execution.recipes` with explicit `execution.recipe` and
`execution.lock` path/content records, replaces per-file network-step identifiers
with `stage`, records effective environment-supplied UID/GID mappings in
`execution.id_mappings`, and uses formatter receipts with `profile` /
`profile_digest` instead of host-runtime `executable` / `runtime` fields. Emit
and validate version three explicitly; do not silently emit these fields as
version one. Version two is not a supported emit or accept target. The transform
record adds a required boolean `default_skipped`, indicating that the implicit
default encountered no DTB or no bootargs property; an authored transform always
records false for this field. `target_present` records whether a DTB was
supplied.
For `authored`, the transform digest is the SHA-256 of the exact committed
recipe bytes containing that section (also identified by `execution.recipe`).
This binds the section without inventing a YAML reserialization. For `builtin`,
it is the SHA-256 of the builder's exact built-in sequence bytes, including when
the default is skipped. Changes to those bytes change the recorded identity.
`source` is `builtin` or `authored`. Input roles are `bootstrap-tools`,
`carapace-base`, and `firmware-tools`; scute origins are `inherited` or
`produced`. The builder record hashes the running executable. `context.repository` is a provenance locator string, not a trust
anchor: on the CLI it is the local repository path; in the appliance it is the
optional CBOR `location` when present, otherwise the fixed mount path
`/run/pichi/repo`. The host-requested `ref` tip is not stored in inventory —
authenticated identity is solely the resolved `context.commit` (and tree).

Finalization checks inventory output identities against the finalized manifest
and validates every referenced blob before publication. Publication differs by
destination:

- **CLI `--output`:** stage under a temporary sibling of the named destination,
  then publish the completed directory with an atomic no-replace rename onto a
  path that must not already exist.
- **Appliance `/run/pichi/output`:** the virtiofs mount root already exists and
  must be empty at start. Complete every fallible check while staging privately
  in a temporary child directory on that same mount. Only then publish by moving
  each top-level layout entry (`oci-layout`, `index.json`, `blobs/`,
  `inventory.cbor`, `build.yaml`, flat `NNNN.cow` / `NNNN.verity` / `boot.pmi`,
  …) into the mount root with no-replace semantics (refuse if
  any destination entry already exists). If any move fails, delete already-moved
  entries and the staging child so the mount does not retain a partial successful
  image. After a successful publish, remove the temporary child. Virtiofs rename
  within the mount is sufficient for this move-children publish; a whole-mount
  rename is neither required nor possible. The shared “atomic publish” invariant
  means “no successful image exposed until publish completes; failure leaves no
  successful image,” not “one rename of the mount root.”

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

## 12. Validation scope

The implementation follows the ordered sandbox contract. Maintained tests cover committed inputs, locks, typed
DTB operations, pipeline combinations, metadata, deterministic inventory,
publication and disposable-VM lifecycle failures. Distro recipes exercise Fedora
DNF/SELinux, Debian APT/systemd, Alpine APK/OpenRC and NixOS closure construction.
A separate firmware recipe constructs an initramfs and arma PMI/DTB, then boots it.

CI checks native amd64/Arm64 Rust and guest lifecycle behavior. Arm64 hosted guest
tests explicitly record TCG when KVM is unavailable. This does not propose TCG as
the production sandbox. Confidential-computing hardware qualification is deferred
by user decision; attestation remains separate work (build-port transport is
locked in [§1.4](#14-build-appliance)–[§1.5](#15-appliance-build-port-cbor)).

The binary remains standalone and does not create VMs; privileged tests run only
inside disposable VMs. Distro recipes do not imply support for every distribution
and architecture combination. The supported labeling environment disables build-VM
SELinux; an enforcing build VM requires separate qualification. Boot checks stop
at their success markers and do not qualify guest-initiated shutdown.

## 13. Remaining decisions

The design uses gix: recipe and lock verify immediately; other context paths
verify on use. Recipe symlinks resolve only within the committed context; hooks
and checkout filters do not execute. The lock and
inventory wire formats, YAML values, device exposure, and supported firmware
measurement contracts are specified above. Temporary writable mappings use
kernel deferred removal and scratch leases for in-process cleanup after failed
activation; abrupt conglobate death is handled by disposal of the build VM. Ordinary tmpfs allocation failures propagate as build failures.

Remaining ecosystem decisions are authentication of the intended Git identity,
creation/override of components beyond carapace and firmware, additional
firmware signing/resealing contracts, and attestation (appliance build-port
transport is already locked in [§1.4](#14-build-appliance)–[§1.5](#15-appliance-build-port-cbor)).

Sandbox policy is specified in section 14. Section 14.4 locks the environment
labeling exception to a pure-Rust Aya program with Rust load, attach and pin,
owned by the trusted environment. Exact capability/syscall/ioctl profiles,
fixed internal deadline values, internal formatter/property schemas, both-root
labeling admission, identity allocation, and runtime mount mechanics are
implemented under that contract. The build-appliance image, build-port CBOR and
virtiofs mount contract are locked in [§1.4](#14-build-appliance)–[§1.5](#15-appliance-build-port-cbor);
full engine reshape and confidential-platform qualification remain separate.
Fully unprivileged storage assembly is outside the selected design.

The initial scope excludes remote Git retrieval, uncommitted working-tree
inputs, submodules, Git LFS, cross-architecture builds, build-time lock updates,
output-cache mutation, firmware discard, partial PMI/DTB replacement,
transform-default inclusion, regex match-count options, whole-node operations,
and execution of `platform` section. Caller working-tree policy is outside this
interface.

## 14. Sandbox lifecycle

The build proceeds in this order; each failure aborts publication:

1. Validate the request, resolve the commit, load recipe and lock from the
   object store, and resolve and verify every required local image input.
2. Capture external network inputs if needed. Allocate the protected RAM
   workspace and execution runtime only when stages require execution.
3. Execute bootstrap, carapace and firmware in order, passing stage outputs
   forward. Bootstrap formats a new snapshot over dm-zero; carapace produces
   one scute for all its directives; firmware replaces the PMI/DTB pair.
4. For each directive, mount assigned roots. COPY uses lazy verify-on-use
   context bytes; RUN constructs a fresh sandbox, executes within its timeout,
   and stops and reaps all descendants. Remove temporary mounts, flush and
   unmount before sealing stage output or releasing storage.
5. Close the execution runtime. Transform the DTB, assemble the complete OCI
   layout, write the transitional flat `build.yaml` (delta scutes only) and
   deterministic CBOR inventory, and verify staged output.
6. Finish fallible workspace/context cleanup, then publish without replacing
   an existing destination: CLI `--output` uses an atomic no-replace rename of
   the completed directory; the appliance empty `/run/pichi/output` mount uses
   the validated move-children publish in §11.2 (no layout entry becomes visible
   until every fallible check has succeeded; a failed move cleans the mount).

On failure, stop and reap commands before releasing their dependencies. Preserve
both operation and cleanup errors. If quiescence or cleanup cannot be established,
reject publication and retain resources until the environment destroys the VM.
RAII owns resources; destructors cannot certify successful finalization.

### 14.1 Trust model and scope

1. Conglobate's engine binary runs as initial-namespace root when privileged
   stages execute. It verifies inputs, interprets recipes, assembles storage,
   owns execution and cleanup, finalizes images, and emits inventory. The
   types-only library has no VM transport or init dependency. Appliance boot
   (systemd PID 1, static virtiofs, build-port console) is specified in §1.4; the
   engine itself does not implement virtiofs or a mount generator.

1. Production deployment is the dedicated disposable appliance VM with
   conglobate as its only major workflow. An independent environment
   controller shuts down the VM on conglobate crash or unexpected exit. No
   new build or resource reuse is allowed in an abandoned VM. A separate
   guardian and recovery across invocations are not required. Qualify
   shutdown behavior in the execution environment. Standalone CLI tests may
   run the same engine without that appliance.

1. Build commands are untrusted, including namespace root. Trust the build
   kernel, supervisor, environment-provided BPF LSM labeling exception and
   image-resident formatting tools selected by the internal map.
   Native filesystem drivers execute inside the trusted kernel. There is
   no userspace filesystem worker in the selected design. A filesystem
   driver vulnerability can compromise that shared kernel.
   Verification of an image's identity is not proof its filesystem is safe
   to parse. Include hostile filesystem inputs in the kernel threat model.

1. The root supervisor owns block devices, mounts, formatting and
   publication. Commands run in separate mapped user/mount/PID/IPC/UTS/net
   namespaces and receive only assigned filesystem views. They must not
   alter supervisor state, inputs, inventory, control interfaces, output
   staging or other builds. Never expose backing block devices to commands.
   Attestation must identify the kernel, policy, formatter and dependencies,
   not merely the conglobate executable.

1. The design must work inside confidential VMs on Intel TDX, AMD SEV-SNP,
   and Arm CCA, without nested virtualization. Use native command
   execution, Linux namespaces, idmapped kernel mounts and environment-provided BPF LSM labeling exception. UML and
   nested KVM are not the baseline; software CPU emulation is not the
   baseline either. Hardware qualification on all three platforms is
   deferred by user decision. Ordinary VM/Arm64 CI tests do not establish it.

1. Conglobate forbids unsafe Rust and uses safe library interfaces for
   privileged operations. This does not assert that dependencies contain
   no unsafe internals. Experimental Python/C harnesses are not production
   implementation templates. Bubblewrap in a prototype does not select
   an external runtime for production. Compare established implementations
   and audit the licenses of the exact dependencies actually distributed.

### 14.2 Resolve and verify all supplied inputs before execution

1. Accept a local Git repository, full commit identity (CLI) or resolve
   appliance `ref` → commit on the mounted repo, context subdirectory,
   read-only image cache, explicit context-relative recipe and lock paths,
   and a new output destination for the CLI (`--output` must not already exist),
   or the empty appliance mount at `/run/pichi/output` into which layout entries
   are published (see [§1.5](#15-appliance-build-port-cbor)). Neither recipe nor
   lock has a default filename. Require
   committed files resolved within the selected context; reject absolute
   paths, escapes and missing/non-file inputs. Record selected paths and
   exact content identities in inventory. Load recipe and lock immediately
   from the object store; resolve COPY and other context paths with lazy
   gix verify-on-use. Do not clone, do not treat a working-tree checkout as
   SoT, and do not eagerly capture the entire context tree. Commands never
   receive the context. Use the Git, lock, and host-architecture rules in
   DESIGN.md. Working-tree policy for callers is outside this interface.

1. Resolve recipe references only through the committed, architecture-aware
   lock, pinning the top-level multi-architecture index and verifying its
   referenced content. Missing artifacts or blobs are errors. Conglobate
   does not fetch Git content, resolve tags remotely, or download extra
   image artifacts. Recipe commands explicitly granted networking may
   download packages; this is distinct from input resolution.

1. Parse one YAML recipe mapping and validate all stage sections before
   execution. The fixed order is bootstrap, carapace, firmware, transform;
   platform remains unsupported. Key order never determines execution.
   Reject duplicate/unknown keys, invalid types and null stage values.
   Omission retains pass-through/default behavior; explicit {} and []
   express empty sections subject to their input requirements.
   Check supported filesystem/backend and runtime prerequisites, required
   image components, finite command timeouts, variable availability when
   evaluated, and DNS configuration for network-enabled commands. Reject
   unsupported behavior rather than silently weakening the sandbox.

1. Require and capture --network-namespace when any step enables networking
   as specified in section 14.8. Read --resolv-conf PATH once if networking is needed and retain the
   selected bytes privately. The argument is mandatory if any run step
   has network: true. An unreadable or invalid selected file is an error;
   there is no implicit host-file or public-resolver fallback. Resolver
   configuration and its digest are deliberately OMITTED from inventory.
   The environment supplies resolvers reachable from the command network;
   a supervisor-loopback address does not become reachable by copying it.

### 14.3 Assemble storage and the build workspace

1. Keep immutable carapace layers cache-backed. Verify the chain and use
   verified reads; do not copy every immutable layer into tmpfs. Merely
   hashing a mutable cache file once is insufficient. Later mutation must
   produce verification failure, never silently change accepted inputs.
   Decoding or format-required materialization may need temporary storage;
   that is not permission to replace cache-backed layers with blanket RAM
   copies. Retain required handles until every consumer is finished.

1. The supervisor assembles verified carapace layers over the required
   dm-zero base and creates writable snapshot storage above them. It owns
   loop/device-mapper resources, names, handles, leases, and recovery.
   Mount namespaces do not namespace these kernel objects. Concurrent
   builds must not collide or remove each other's resources.

1. Use one private build-wide tmpfs for mutable snapshots and scratch:
   disposable tool roots, newly generated
   output-layer storage, runtime state, resolver bytes, and intermediate outputs.
   Use ordinary tmpfs behavior, with no disk fallback. The environment
   configures capacity and resource limits; do not add per-step memory,
   swap, or tmpfs-size policy to recipes. Allocation/write failures fail
   the build. Final artifact destination storage is separate.

1. Mount assigned filesystems with trusted kernel drivers. Only one writer
   implementation may own a writable backing volume at a time. Retain the
   mounts and device handles through command quiescence and clean
   filesystem finalization. Raw block access is supervisor-only, including formatting.

1. Bootstrap starts a new writable snapshot layer directly over dm-zero.
   Format through its block-device view: formatting and population write
   into this layer from the outset. No temporary rootfs image, image import,
   or image-to-scute conversion is part of bootstrap. After clean teardown,
   seal/hash the resulting scute and package the new carapace.

### 14.4 Mount filesystems under the environment-provided labeling exception

1. Use kernel filesystems supporting the required idmapped-mount and metadata
   semantics. Ext4 is validated initially; qualify additional filesystems
   separately. Fail explicitly if the selected driver, trusted formatter,
   idmapping, BPF LSM, BTF or required hooks are unavailable. No fallback to
   command CAP_SYS_ADMIN, a different filesystem or an unreviewed backend.

1. The supervisor mounts the filesystem and creates an idmapped view for
   the command namespace. Keep the superblock supervisor-owned; it does
   not need to belong to the command user namespace. Map image IDs to
   unprivileged outer IDs while preserving the correct IDs on disk. Cover
   all supported image IDs; reject unmapped IDs. The environment reserves and
   supplies contiguous parent ranges using --uid-map PARENT_START:COUNT
   and --gid-map PARENT_START:COUNT. Both start at image ID zero and are
   required for execution; there is no built-in range or allocator. Exclude
   parent root and invalid IDs; keep these ranges disjoint from environment
   service accounts and other workloads. Record the effective pair in the
   inventory and use it consistently across all executing stages. Retain normal
   kernel ownership, mode and ACL permission checking. Use nosuid,nodev;
   storing file capabilities/setuid bits must not grant build-host privilege.

1. The trusted environment loads a fixed BPF LSM exception before
   conglobate starts. It delegates the generic capability check for exactly
   security.selinux through inode_xattr_skipcap. There are no per-command
   registrations, subject/filesystem maps, target guards or activation
   transitions. Conglobate does not compile, load, attach, update or remove
   this policy and does not depend on libbpf. The environment retains the
   pinned program for the lifetime of the VM. Conglobate's only labeling
   role is LabelProbe admission: check required behavior under the
   restricted command credentials, and do not install the policy.

1. Implement that environment exception as a pure-Rust rewrite using Aya.
   The trusted environment owns the LSM program and the load, attach and
   pin path; both are Rust. This is the chosen environment path for
   building the program and for load, attach and pin. Do not implement
   the exception as the C program tests/environment/labels.bpf.c loaded
   by tests/environment/loader.c, and do not revive the retired
   libbpf-rs wrapper around that C object. A C BPF toolchain (bindeps)
   or a precompiled object is not this path. Conglobate still does not
   compile, load or attach the program.

1. Until the Aya program and its load, attach and pin path exist, keep the
   current C fixture as the interim qualification harness for disposable-VM
   tests. It is not linked into the conglobate binary and it is not the
   production environment. When the Aya path exists, retire
   tests/environment/labels.bpf.c, tests/environment/loader.c and the
   related C loaders, including tests/distro/labeling-fault/loader.c. The
   labeling-fault hook remains a test-only second program; retiring its C
   loader does not add a second production policy.

1. Match the complete attribute name, including termination; truncated
   prefix matches are forbidden. Preserve earlier LSM results. Other
   security attributes retain normal checks. The build VM must have SELinux
   disabled; image label strings belong to the target OS's independent
   policy and must not confer authority in the build environment. The
   exception is not a grant of CAP_SYS_ADMIN or SELinux administration.

1. Ordinary file write-mode denial is not a labeling integrity boundary:
   security.* attribute checks can bypass that DAC check. Commands must
   not reach mutable supervisor/cache/context/control inodes or their
   descriptors. Assigned read-only views must reject label mutation.
   Keep bpffs, policy controls and privileged handles inaccessible.
   The same fixed exception supports both bootstrap writable roots.
   Enforce this boundary through private mount construction, pivot_root
   and old-root detachment, confined runtime mount targets, private proc
   and non-dumpable trusted helpers, and closure/audit of all non-stdio
   descriptors before image code. Command stdout/stderr are pipes relayed
   by trusted setup, not direct handles to supervisor output files;
   stdin is the assigned null device. Read-only mounts protect supplied
   immutable views. Seccomp and capability restrictions prevent commands
   from obtaining new mounts or joining outside namespaces. Validate this
   combined boundary again with the global exception active.

1. Validate the required labeling behavior before admitting recipe commands
   under the restricted command credentials. Missing or incompatible
   environment support fails the build, with no capability fallback.
   Do not infer support from a familiar pin pathname or a successful
   supervisor-root write. This is a behavioral admission prerequisite,
   not dynamic policy installation. Its production implementation and
   failure tests must accompany the lifecycle migration.
   Complete the cgroup-join acknowledgment before launching the restricted
   leaf. Probe in that contained leaf after capabilities and seccomp are
   applied, before reporting readiness or receiving command admission.
   Check both assigned filesystems in bootstrap using anonymous inodes;
   close all probe handles before readiness and the final descriptor audit.

1. Retain namespace handles needed for idmapped views and command setup,
   process containment, mounted volumes and explicit fallible cleanup.
   Remove identity pins and rollback states used solely for policy
   registration. Stop/reap all descendants before storage finalization;
   cgroup joining and admission handshakes remain necessary. Failure to
   prove quiescence rejects publication and requires VM disposal. There
   is no policy revocation step and no numeric policy-identity reuse race.
   Supervisor death still must not expose privileged handles or permit
   storage reuse before the independently triggered VM shutdown.

### 14.5 Execute the assembly pipeline without changing its inheritance rules

1. The bootstrap section selects a tool carapace and declares the output filesystem
   type, logical size and supported typed properties. Trusted formatting
   tools create that filesystem on the new zero-backed writable carapace
   layer before commands start. Never pass arbitrary recipe-provided shell
   commands or formatter arguments to privileged tooling. Validate properties
   against the selected backend and record effective configuration and tool
   identity as build inputs (with an explicit inventory schema revision).

   Commands see the disposable writable tool carapace at / and the fresh
   output filesystem at /mnt. They can install tools into / and populate
   /mnt, for example using dnf --installroot=/mnt. COPY destinations follow
   this same sandbox layout. Constrain /mnt setup against the pinned root;
   it is a reserved output mount during bootstrap. Tool modifications persist
   between its steps but are discarded at stage completion. No tool layers
   or firmware components are inherited. Deliberately copied tool files can
   contribute output bytes. Stop writers and cleanly unmount both views;
   seal the output layer directly, without import or image conversion.

1. The carapace section extends bootstrap output, or its explicit from image when
   bootstrap is absent. from is forbidden when bootstrap supplied the
   input. Preserve all inherited image components, not just the scutes.
   Empty/omitted stages and firmware-only images follow DESIGN.md.

1. The firmware section produces a replacement PMI/DTB pair together, commonly
   using arma. It may use a different tool carapace from the output base.
   Commands use a disposable tool root at /, without a special /mnt
   output mount. into names the PMI and DTB paths inside that root;
   extract them through confined handles only after all writers stop.
   Omission propagates inherited firmware. Do not add a firmware-discard
   operation or permit replacement of only half of the pair.

1. The transform section edits DTB properties in the supervisor with safe libraries.
   It does not require a command sandbox. An authored sequence replaces
   the built-in sequence completely; transform: [] disables it. No default
   inclusion operation. Regex replaces all matches, leaves zero matches
   unchanged, and fails on a non-string. Missing reads fail unless guarded
   by property-exists; set can create nodes; remove of an absent property
   fails. Missing DTB fails for authored transforms, but skips the implicit
   default. The implicit corium edit tolerates absent bootargs and edits
   existing corium= arguments, without assuming every firmware is Linux.
   See DESIGN.md for exact typed operations and default syntax.

1. Expose $CARAPACE consistently after carapace production/selection as the
   output carapace root, not the firmware tool root. Referencing it when
   unavailable fails. Expand string values with the documented dollar
   escaping. The platform section remains unimplemented; do not add it
   as part of this sandbox redesign.

### 14.6 Keep COPY and output extraction controlled

1. Commands never receive a context mount. COPY selects committed files
   using the existing Docker-compatible semantics. Copy into a filesystem
   through its kernel mount, while command writers are stopped. Use
   scoped handles and root-confined path resolution; symlinks and metadata
   must not redirect supervisor operations outside the target.

1. COPY is a supervisor-owned operation, not an image-authored script with
   context access. Use a controlled mount session without command writers,
   root-confined path resolution and narrowly scoped handles. Complete it
   before the next run. Apply the same rules to disposable tool roots and
   bootstrap output roots; do not expose the context object store to commands.

1. Extract declared firmware outputs only after commands and
   descendants have stopped. Validate types, bounds, paths, and formats.
   Do not execute image-provided cleanup or validation helpers with
   supervisor authority. COPY into a persistent layer is persistent data;
   deleting it in a later layer does not erase the earlier layer.

### 14.7 Start a fresh command sandbox for every run step

1. Remount assigned filesystems for every command step.
   Create a fresh command sandbox; complete teardown before advancing.
   Persistent image changes survive through the writable storage, not
   through surviving processes or long-lived filesystem mounts. The externally
   owned enabled network namespace is intentionally shared across steps.

1. Execute as UID/GID 0 inside the mapped command user namespace, with
   private mount, PID, IPC and UTS namespaces, a separate network namespace
   selected according to section 14.8, and private mount
   propagation. Pivot to the assigned image root and detach the old root.
   No initial-user-namespace root authority, context, backing device,
   /dev/fuse, cache, supervisor executable, or outside-root descriptors
   may reach commands. Namespace root supplies filesystem-build semantics,
   not unrestricted host-root capabilities.

1. Use a tested capability allowlist and syscall/ioctl restrictions, with
   no_new_privs. CAP_SYS_ADMIN MUST be absent from all five command
   capability sets, including the bounding set. Preserve supported chown,
   ACL, mode, credential, file-capability and label operations through
   idmapping and the environment-provided labeling exception. Block namespace creation/join and
   mount APIs; a new user namespace would grant new scoped capabilities.
   Restrict devices, ioctls, keyrings, process inspection and descriptor
   access. Validate the actual profile on amd64 and arm64; the prototype
   allowlist is not a production compatibility contract.

1. Supply private /proc for the command PID namespace, minimal private /dev
   with approved devices, private devpts and /dev/shm, and empty read-only
   /sys. Never bind the supervisor's device tree or its device-node inodes.
   Constrain runtime mount targets against the pinned image root before
   executing commands. Do not hide COPY inputs under an automatic /tmp
   mount or silently change cross-step /run persistence. Temporary runtime
   injection must leave no supplied resolver data or scaffolding in output.

1. Run /bin/sh -eu -c, cwd /, null stdin, and explicit diagnostic output.
   Clear the caller environment; supply only the documented PATH, HOME,
   LANG, and applicable CARAPACE. No automatic credential/proxy forwarding.
   Namespace init must reap descendants. No process may outlive the step,
   including a daemon whose shell already exited successfully.
   Resident init reports shell status before namespace-exit cleanup through
   an owned status-only channel directly to the supervisor. The receiver
   independently reports the namespace helper exit after local cleanup;
   the supervisor cross-checks it against the normalized shell status.
   Keep init non-dumpable and this endpoint
   close-on-exec; no image command may inherit or obtain it through procfs.
   It grants no storage/control authority and is not a cleanup receipt.

1. Every run has a finite positive timeout. Default: 5 minutes. Recipes
   may override it with duration strings, for example timeout: 30m.
   Reject zero, infinite, invalid, or overflowing durations. The timer
   covers command execution only, from release to execute through command
   completion; remaining descendant cleanup belongs to teardown. It does
   not include setup, environment validation, mounts, or teardown. Timeout fails
   the build and starts cancellation; partial results are not accepted.

### 14.8 Networking and DNS

1. Networking defaults to disabled. Offline commands receive a fresh
   disconnected network namespace. Enabled commands join the externally
   supplied --network-namespace PATH, captured once as an owned descriptor
   before command execution. Reject missing/non-nsfs input and the supervisor's
   own network namespace. Reject wrong namespace types through setns(CLONE_NEWNET)
   in trusted setup before admitting commands. The environment supplies initial-user-namespace
   ownership; conglobate does not introspect it. Capture precedes command
   user-namespace creation and restricted capabilities exclude network
   administration. Join during trusted setup; no handle or administration
   authority reaches command code.

1. The environment owns the enabled build namespace and its interfaces,
   addresses, routes, NAT, firewall policy and lifetime. It provides outbound
   NAT with replies allowed, no unsolicited inbound connections or inbound
   forwarding, and access to all reachable destinations including private
   networks. Conglobate neither provisions nor tears down these resources
   and does not require supervisor-namespace forwarding sysctls. The
   environment dedicates the namespace to this build. Sequential enabled
   steps reuse it; connection tracking and other network state can persist,
   but no command processes may survive a step. No unrelated services or
   concurrent builds share the namespace. Systemd is an optional environment
   implementation, not a dependency of the binary.

1. Namespace separation isolates loopback and abstract Unix sockets, not
   services listening on routable supervisor addresses. This design does
   not promise to block access to every address belonging to the build VM.
   Supervisor controls and attestation endpoints must not listen on the
   command-reachable network. The surrounding environment owns upstream
   connectivity and externally reachable services.

1. Mount the captured --resolv-conf contents read-only at /etc/resolv.conf
   for network-enabled commands. Follow existing symlinks only within the
   command root, including targets under /run. Reject escaping paths,
   loops, and incompatible target types. Use confined temporary mount
   scaffolding where needed; preserve existing image files/symlinks and
   remove created placeholders after unmount. Failure to restore the
   promised output state fails the build. Qualify dangling targets and
   concurrent rename/replacement cases before shipping this behavior.

1. Do not persist the supplied resolver file into output layers or infer
   a DNS relay. Conglobate starts no DNS server. The environment supplies
   a usable resolver file and routes. Supplying resolv.conf does not supply
   NSS modules, nsswitch policy, credentials, proxies, or /etc/hosts entries;
   image authors supply their own required userspace configuration.
   Offline steps receive no injected environment resolver configuration.

### 14.9 Filesystem compatibility contract

1. Preserve correct target UID/GID values, permissions, ordinary extended
   attributes, ACL access and inheritance, file capabilities, and SELinux
   labels. The target OS policy is independent of the build VM policy.
   Labels are image metadata; commands must not administer the build VM's
   SELinux policy. Successful xattr readback alone is insufficient: check
   access decisions and the bytes/behavior after independent target boot.

1. The tested foreign-label path uses a build kernel with SELinux disabled.
   SELinux in the build VM is not a requirement; isolation remains one.
   Do not equate permissive mode with disabled/absent SELinux. An enforcing
   deployment must separately demonstrate the required foreign-label
   behavior without weakening command containment.

1. Initially unsupported: access to trusted.* xattrs, persistent device-node
   creation, and changing immutable/append-only flags, including clearing
   inherited flags. Inherited flags can block edits or deletion. Fail with
   useful diagnostics; do not silently fake success or discard metadata.
   Temporary sandbox /dev is distinct from persistent image device nodes.
   Named pipes are supported: restrict mknod/mknodat by mode to FIFO only;
   character and block device creation remain denied.
   Untouched inherited bytes remain inherited; this is not a promise to
   copy inaccessible metadata through file-level operations.

1. Retain the explicit initial restrictions above until separately reviewed;
   native driver support alone does not broaden command authority. Qualify
   each formatter, driver, idmap and metadata combination, including the
   target image's boot behavior.

### 14.10 Own execution, failure, and cleanup end to end

1. Conglobate owns setup helpers, command containment, namespace init and
   descendants. Helpers prepare isolation and launch untrusted commands;
   they are not independent recovery supervisors. Retain cancellation
   authority outside the command sandbox. Do not rely solely on process
   groups, launcher exit or numeric PID paths. The environment handles
   abrupt conglobate death by shutting down the dedicated VM.

1. Setup and teardown have finite FIXED INTERNAL DEADLINES, chosen and
   tested as implementation constants. They are not recipe or CLI knobs
   and do not consume command timeout. Exceeding either fails the build.
   Start teardown's budget when command completion or observation failure
   is observed. Retain that cutoff across delayed cleanup calls and retries;
   neither may start a fresh budget.
   Cancellation or an environment deadline may end work earlier. A fixed
   deadline bounds waiting/acceptance; it cannot guarantee the kernel can
   immediately reap a task stuck in uninterruptible I/O.

1. On normal completion, stop/reap all descendants, then remove
   runtime/resolver mounts, release borrowed network handles without modifying
   the external network, flush and cleanly unmount assigned
   filesystems, and release references before sealing or the next step.
   Detect and propagate storage/writeback errors. Preserve primary failure
   and cleanup diagnostics independently; process exit is not durability.

1. Command error, timeout, filesystem error, helper death,
   cancellation or setup/teardown deadline rejects the build. Quiesce
   commands before mount teardown. Do not accept a forced/lazy unmount
   as clean finalization. Retain recovery ownership if cleanup cannot finish.
   Never silently resume a failed command on remounted storage.

1. Incomplete cleanup must not authorize publication or unsafe identity/
   resource reuse. Keep ownership while the process remains alive, report
   failure and require VM disposal if teardown cannot complete. Validate
   normal completion, partial setup, timeout, helper death, blocked I/O,
   supervisor death with independent VM shutdown, and output rejection.
   Shared-host recovery and concurrent builds inside one VM are out of scope.

### 14.11 Finalize and publish

1. After writers are gone and filesystems are cleanly finalized, seal
   and hash writable output layers using the carapace APIs; firmware
   extraction and DTB transformation follow the pipeline contract. Hash
   the finalized bytes that will be published, not subsequently mutable
   transport copies. Do not treat a clean process exit alone as evidence
   that filesystem writes succeeded.

1. Emit a self-contained OCI image-layout artifact and deterministic CBOR
   inventory binding the verified context, lock, image inputs, recipes,
   effective build behavior, and final outputs. Include every referenced
   output blob, including inherited ones. Resolver contents/digest are an
   explicit inventory exclusion; network-step permission is recorded.
   Network acquisition is not claimed to be reproducible or exhaustive
   input accounting. Version any schema change explicitly; do not add
   fields silently to the strict existing inventory wire format.

1. Require a new output directory. Stage the complete artifact, verify its
   identities, and publish atomically without replacement. Failures emit
   diagnostics, not a successful artifact. Cache mutation/import and
   attestation belong to external integrations. The appliance may attest
   the exact inventory bytes and trusted execution environment later.

### 14.12 Validation requirements

1. Before accepting production output, qualify the actual integrated
   kernel/device contract, dual-mount bootstrap, full tool-root launch, trusted COPY
   and export, capability/seccomp policy on both architectures, ID reuse,
   keyring/FD closure, helper death, supervisor death with VM shutdown,
   fixed deadlines, blocked I/O, independent build VMs, clean/error
   finalization, and publication ordering.
   Include benign owned-object positive/negative containment tests and
   malformed-input/parser tests; passing them is not proof of absence
   of vulnerabilities. Do not substitute historical markers for new runs.

1. Validate outbound NAT, reply-only inbound behavior, all-reachable/private
   destinations, supplied DNS (including image NSS), resolver symlinks and
   missing targets, absence of persistent injection, and network cleanup
   under failures in the final kernel-mount topology.

1. Build Fedora, Debian, Alpine, and NixOS images through representative
   bootstrap, layering, and firmware workloads. Exercise package installs/
   upgrades, ownership and ACL changes, initramfs/kernel tooling, relabeling,
   cleanup, and image boot. Fedora must boot with functional SELinux under
   its own policy. Report accepted filesystem limitations where workloads
   encounter them; do not silently broaden the support claim.

1. Deferred qualification (user decision): run the composed design in
   Intel TDX, AMD SEV-SNP, and Arm CCA deployment
   environments. Measure startup/step costs and compatibility, with no
   nested-virtualization assumption. Appliance transport and attestation
   qualification are separate from the standalone binary's tests.

## 15. Rust architecture

The crate exposes a **types-only `[lib]`** (including OCI/image output types
retired from `pichi-artifact`) and a **`[[bin]]` engine**. Engine modules remain
private to the binary. Opt-in VM fixtures compile the same engine modules into a
separate test binary. Downstream `pichi` depends on the lib for shared types.

### Inputs and workflow

The standalone CLI or appliance supplies committed Git objects (after ref
resolution), a read-only local image cache, UID/GID ranges (CLI flags or
appliance image hardcodes), optional network namespace and resolver file (same
split), the fixed labeling exception and independent VM disposal. Formatting
uses the internal filesystem-name → formatter map; there is no host
`--formatter-runtime` input. Conglobate performs no artifact downloads, Git
clone/checkout-as-SoT, network setup, policy registration or attestation. It
has no dependency on the pichi binary; pichi depends on this lib.

`BuildRequest`, `ResolvedBuild` and `BuildPlan` progressively validate inputs.
Recipe and lock load immediately from the object store; COPY and other context
paths use lazy gix verify-on-use without exposing a context mount. Locked
multi-architecture indexes select only the host architecture. Image components
and verified chain roots remain separate from live device mappings.

`Pipeline::execute` sequences bootstrap, carapace and firmware directly. It owns
one RAM workspace and execution runtime, borrowed by stages. Bootstrap formats
its new writable snapshot over dm-zero **outside the command sandbox** and uses
separate tool/output mounts. Carapace directives share one writable snapshot and
produce one scute for the whole stage. Firmware uses disposable tools and
extracts the PMI/DTB pair. Tool inputs contribute inventory provenance, not
accidental output ancestry.

`StageOutput` holds results without command/mount authority. Runtime cleanup
precedes DTB transformation, image assembly, deterministic CBOR inventory and
publication. Pass-through builds use the same finalization without allocating
an execution runtime. Publication happens only after all fallible checks. CLI `--output`
uses a no-replacement atomic rename of the completed directory. The appliance
`/run/pichi/output` mount cannot be renamed as a whole; after private staging
and validation it publishes by moving each top-level layout entry into the
empty mount root with no-replace semantics (§1.5, §11.2). If any publish move
fails, remove already-moved entries and staging so no successful image remains
exposed. Either path: no successful image is exposed after a failed build.

### Ownership and Rust states

Application Rust forbids unsafe code. Methods belong to types that own their
invariants. Private fields, sealed marker traits, immutable validated values and
consuming transitions express what each operation may do. Important examples:

- `Volume<Unmounted>` and `Volume<Mounted>` retain mapping/mount ownership.
  `VolumeView` borrows a root from storage actually retained by the step.
- `Step<Ready>` and `Step<Running>` constrain execution. The supervisor owns
  the direct init gate; failed start retains the entire ready step for cancellation.
  Cancellation, reaping and descendant draining form one private cleanup sequence
  in `ContainedHelper`, using the original teardown deadline.
- `CommandStorage` shares preparation, transfer, COPY and finalization between
  single-root commands and bootstrap's paired roots. Networking is owned directly
  by the step, without another storage wrapper or state machine.
- Internal formatter selection, staging, execution and completion own distinct resources.
- I/O operations use `io::Result` and direct propagation. The pipeline uses
  `anyhow` for stage/directive context and combined operation/cleanup diagnostics.
  No failed build returns reusable storage or a recovery object. Private process
  transition errors may carry a child to the immediate cancellation path; these
  never escape as a build recovery API.

RAII provides ordinary scoped cleanup; destructors cannot prove quiescence or
successful writeback. Explicit fallible completion performs those checks.
Terminal failure retains diagnostic truth and requires VM disposal when cleanup
cannot finish. There is no guardian process or persisted recovery state machine.

### Storage

Immutable aligned scutes stay cache-backed. RAM-backed copies are used where
compression or padding requires materialization; writable layers and filesystem
workspaces are always RAM-backed. Cache identity checks and authenticated reads
remain distinct protections. Device-mapper and loop owners retain descriptor and
mount-namespace identity through partial setup and explicit teardown. Unresolved
live resources remain retained until VM disposal. Deferred device removal is a
destructor fallback, never evidence of successful close or permission to seal.

The carapace format uses a 64 GiB dm-zero origin. Bootstrap's configured ext4
capacity is a filesystem capacity within that geometry, not a format change.
The initial backend accepts aligned 16 MiB–64 GiB capacities and typed ext4
properties. Trusted mke2fs uses 4096-byte filesystem blocks. Finalization trims
unused snapshot capacity, uses ecosystem wire types and seals the scute directly;
there is no image-to-carapace import/conversion step.

Scute wire parsing belongs to devmap-snapshot. Conglobate owns activation and
fallible cleanup using devmap/lodown. A read-only file with deferred removal alone
cannot prove successful finalization. It does not duplicate the
wire codec. The pinned rustix revision supplies the safe borrowed-descriptor
mount_setattr API (upstream PR 1630); the pin is intentional, not a mutable branch.

### Command boundary

The supervisor and kernel are trusted. Commands receive mapped namespace-root
credentials, private mount/PID/user and other isolation, private proc/dev/devpts,
restricted capabilities and a native-ABI seccomp allowlist. They never receive
SYS_ADMIN, backing devices, the context, supervisor resources or host terminal
handles. Command stdout/stderr are pipes relayed by trusted code; stdin is null.
PTY queries/unlock/raw-mode settings and caller-local parent-death signals support
Nix builders. Terminal injection/control and namespace/mount creation stay denied.

The environment's fixed BPF LSM exception permits exactly security.selinux
labeling without making the target filesystem's policy the build VM's policy.
The trusted environment builds, loads, attaches and pins that exception with
Aya (pure Rust), as specified in section 14.4. Conglobate probes labeling
under the actual command restrictions before admission, including both
bootstrap roots. That `LabelProbe` check is its only labeling role. It neither
loads nor revokes the exception. Ordinary ownership, ACL, mount and filesystem
checks still apply.

Each command reconstructs mounts/isolation and has a finite recipe timeout
(default five minutes). Setup and teardown use separate fixed internal deadlines.
Descriptor handshakes establish readiness; sleeps are not synchronization.
The supervisor sends init's gate endpoint over the receiver's existing control
channel. Init reports READY directly; the supervisor establishes the execution
deadline before sending START and receives raw shell status directly. The
receiver retains its mount/resolver/diagnostic cleanup duties and independently
reports the namespace helper's exit status after cleanup. The supervisor checks
that exit against init's normalized shell status. Neither status alone certifies
quiescence, unmount or writeback. A missing or inconsistent cleanup report still
triggers cancellation and drain, but cannot authorize storage finalization.

`DescriptorPacket` shares only single-call nonblocking descriptor transport;
protocol owners retain framing, descriptor shape/type checks and wait policy.
Queued datagrams never gain retries. `Deadline` converts an existing absolute
cutoff to a poll duration without renewing it; cross-process conversion remains
in `SetupCutoff`.
Cgroup containment, process identity, cancellation, reaping and descendant drain
precede releasing mounts or sealing storage. Independent environment shutdown
handles supervisor death; another build must never reuse that abandoned VM.

Networking is offline by default. Enabled commands join an environment-supplied,
validated and pinned namespace and receive captured read-only resolver bytes.
The environment owns outbound NAT, ingress policy, DNS and namespace lifetime.
Resolver mounts follow symlinks only within the command root and restore prior
image state. Conglobate implements no NAT allocator, resolver service or networkd
integration.
