# Conglobate image build design

Status: design target for the locked build-VM redesign (docs-first).
Requirements use “must” or direct statements of behavior. The crate is a
types-only library plus a binary engine; the standalone CLI remains first-class
and VM-free for tests. The [sandbox lifecycle](#14-sandbox-lifecycle) and
[Rust architecture](#15-rust-architecture) are specified below.
Deferred ecosystem work is collected in [Remaining decisions](#13-remaining-decisions).
[Validation scope](#12-validation-scope) describes maintained coverage and platform limits.
Implementation of the full engine reshape is separate from this design lock-in.

The 28 October 2026 decision in [§11](#11-artifact-and-build-annotations)
locks build binding annotations and removes `inventory.cbor`. User-facing YAML
and prose call the document a **plan** and the selected path in a commit the
**context**. The target CLI uses `--plan`; build-port fields are `plan` and
`context`. This is a documentation lock: code, shared Rust types/modules and
fixture filenames still use legacy names until the follow-up implementation.
The existing `src/plan.rs` engine module has a naming conflict to resolve
in the follow-up code tip; no Rust module or file is renamed by this decision.

The approved plan design, dated 27 October 2026, is the multi-stage task
model in [§5](#5-context-and-pipeline) and [§6](#6-execution-plans). It is
the plan source of truth. Tasks are `bootstrap`, `extend`, and `build`. The
lock is a separate document. References are an untagged `Local` | `Remote`
enum. `Filesystem` is a kind-tagged enum. `image:` has a short form and a long
form. Only the image carapace/boot closure runs. DTB rewrite for a new root
hash is automatic, including when assembly binds a root to imported boot
lineage. This replaces the fixed `bootstrap` → `carapace` → `firmware` →
`transform` pipeline. The engine implements the task schema and executes the selected
closure. Provisional artifact grammar choices are documented in README.md and
src/recipe/schema.rs; they are not extra wire locks in this document.

Conglobate builds images from committed source and immutable image inputs. A
bootstrap task formats a new carapace from a tool carapace. An extend task
produces a carapace from a carapace and commands, and carries boot with it. A
build task produces artifacts only. A pichi image is not a task: it is the
assembly of one carapace and optional boot (PMI/DTB). The output is that image
with build binding annotations on its manifest (see [§11](#11-artifact-and-build-annotations)).

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

The builder verifies inputs, interprets plans, assembles storage, formats and
mounts filesystems, enforces execution policy, finalizes outputs, and annotates
the image manifest. Kernel filesystem drivers replace the separately sandboxed
FUSE worker. Commands run in mapped user namespaces with private mount/PID/IPC/UTS/
network namespaces. Idmapped mount views preserve target ownership on disk.
The kernel parses filesystem contents: this increases the shared-kernel attack
surface compared with an isolated userspace worker. Content verification does
not prove that filesystem metadata is safe to parse.

Commands may modify only their assigned working filesystems and explicitly
authorized outputs, and use explicitly provided runtime resources. They must
not access or interfere with builder processes, backing devices, namespace
handles, control interfaces, image metadata, output staging, or immutable inputs.
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
The finalized image bytes must remain stable through publication.
Future attestation must bind the trusted execution environment, including the
kernel, environment-provided labeling policy, appliance image (and its internal
formatter profiles) and effective configuration, to the image digest; measuring
conglobate alone is insufficient.

The implementation uses kernel ext4 mounts and an environment-provided labeling
exception. The trusted environment implements that exception as a pure-Rust Aya
program and a Rust load, attach and pin path (section 14.4). Conglobate does
not compile or load it. Confidential-hardware qualification remains deferred;
tests do not prove absence of sandbox vulnerabilities.

The sections follow a build from input selection through execution and output
verification. Read the architecture and input sections first, then the pipeline
and plan reference. Resource ownership, measurement, and the image manifest
define the implementation's trust boundaries.

## Terminology

| Term | Meaning in this design |
| --- | --- |
| Image | A manifest and its content-addressed components: a carapace, a firmware pair, or both, with associated configuration and metadata |
| Scute | One immutable filesystem block layer stored as a copy-on-write (COW) object and protected by dm-verity |
| Carapace | An ordered chain of scutes assembled into a read-only block device; its top root hash binds the chain |
| Tool carapace | A carapace supplying the filesystem in which a task's commands execute. Changes to it are disposable unless the task explicitly exports them. It does not become output ancestry because commands ran inside it |
| Base image | The carapace an `extend` task takes through `from:`, together with boot when that task re-exports it. A tool carapace is not a base image |
| Task | A named, kind-tagged unit of plan work. The kinds are `bootstrap`, `extend`, and `build` |
| Reference | An untagged enum: `Local` names a task in this plan; `Remote` is a fully qualified OCI image. References have no `./` prefix |
| PMI | Portable Machine Image, the boot-payload format consumed by the VM launcher |
| DTB | Device Tree Blob, a binary tree of nodes and byte-valued properties describing the platform and its boot configuration |
| Firmware pair | The PMI and detached DTB. `extend` rewrites the DTB for the new root hash and re-exports the PMI when one is present. Custom PMI/DTB bytes come from an explicit task |
| Boot | The optional PMI/DTB assembled with one carapace. On `extend`, boot rides with the task |
| Corium | The guest boot/runtime component whose `corium=<root>` command-line argument selects the root carapace |
| Arma | The tool commonly used by firmware plans to produce a PMI and detached DTB |
| Context | The committed project subtree containing plans and source files |
| Plan | The user-facing YAML document defining tasks and image assembly |

The OCI manifest digest identifies the image description. A carapace root
identifies its verified filesystem chain. Neither replaces the other: boot
assembled into the image is covered by the image description, while filesystem
assembly uses the carapace root.

## Contents

- [Architecture and scope](#1-architecture-and-scope)
  - [Build appliance](#14-build-appliance)
  - [Appliance build-port CBOR](#15-appliance-build-port-cbor)
- [Standalone CLI](#2-standalone-cli)
- [Git context](#3-git-context)
- [Image locks](#4-image-locks)
- [Context and pipeline](#5-context-and-pipeline)
  - [Tasks](#51-tasks)
  - [Image assembly](#52-image-assembly)
  - [Closure](#53-closure)
- [Execution plans](#6-execution-plans)
  - [References](#61-references)
  - [Task kinds](#62-task-kinds)
  - [exec, from, into, and copy](#63-exec-from-into-and-copy)
  - [Filesystem](#64-filesystem)
  - [Command execution](#65-command-execution)
  - [Rejected Docker behavior](#66-rejected-docker-behavior)
- [Build variables](#7-build-variables)
- [DTB transformations](#8-dtb-transformations)
- [Privileged execution and resource ownership](#9-privileged-execution-and-resource-ownership)
- [Firmware measurement boundary](#10-firmware-measurement-boundary)
- [Artifact and build annotations](#11-artifact-and-build-annotations)
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
   commit (or resolved ref), a context path, plan/lock paths, a
   read-only image cache and an output directory, it produces an importable
   artifact with build annotations without requiring virtiofs, a hypervisor,
   or the appliance image. The supervisor runs as root for storage assembly when
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
2. Load plan and lock immediately from the object store; resolve other context
   reads lazily with verify-on-use (see [§3](#3-git-context)).
3. Load tasks and `image:`. Run only the carapace/boot closure
   (`bootstrap`, `extend`, `build`). Orphan tasks do not run.
4. Bootstrap formats outside the sandbox and populates `/mnt`. Extend
   produces a carapace, rewrites the DTB for the new root hash, and
   re-exports the PMI when present. Build produces artifacts only.
5. Assemble one carapace and optional boot. Assembly is not a task. When
   the image root is bound to imported boot lineage, rewrite the DTB
   automatically. Annotate the final image manifest with the build binding facts.
6. In the appliance, later bind the image digest and TEE/builder to
   attestation evidence.

The annotated OCI image is the Conglobate output. Importing or publishing the
image, transporting it out of a VM, and obtaining hardware attestation are
surrounding integrations.

### 1.2 Implementation constraints

Conglobate uses `#![forbid(unsafe_code)]`. Privileged operations are implemented
through safe library APIs. This is a prohibition on unsafe code in conglobate;
it does not claim that every transitive dependency has no unsafe internals.

Use the ecosystem's storage, carapace, device-mapper, loop-device, and DTB
libraries where appropriate. Library selection and API adaptation remain
implementation work. Do not recreate low-level syscall interfaces or shell out
to system tools for the built-in DTB operations. Plan-authored commands can
of course invoke tools inside their execution environment.

YAML is used for human-authored formats. CBOR is used for machine-generated
formats controlled by this project. Existing external formats, including OCI
JSON and Git objects, retain their native encodings.

### 1.3 Measurability and reproducibility

The builder must identify and consume stable supplied inputs. Git commits,
image manifest digests, and carapace roots provide different identities and
must not be conflated. A repository location or mutable image tag is not an
immutable identity.

Per-step network access is supported. The commit, lock and manifest do not
identify every downloaded byte or promise reproducibility. Network dependency
pinning and reproducible OS/package selection are outside the initial
builder's guarantees. Build annotations do not claim a closed set of all
influences when commands can acquire external input.

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
   (`oci-layout`, `index.json`, `blobs/`). Beside it, the
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
| `plan` | when `ok` is `true` | Path to the plan YAML, always relative to `context` |
| `lock` | when `ok` is `true` | Path to the lock YAML, always relative to `context` |
| `context` | no (success) | Context path within the commit; default `"."` |
| `location` | no (success) | Optional provenance/locator hint for the manifest; not an authenticated identity |

Explicit non-fields on the success response (must not appear; if present, reject):

- `commit` (guest resolves `ref` → commit)
- virtiofs mount tags or mount paths
- `output` destination (fixed at `/run/pichi/output`)
- formatter runtime / mke2fs host manifest
- UID/GID maps, network-namespace path, resolv-conf path
- `op` (request-only)

Plan and lock paths are always relative to `context`. Conglobate opens the
mounted repository at `/run/pichi/repo`, resolves `ref` to a commit, and uses
`/run/pichi/cache` as the read-only image cache.

**Ref resolution (guest):** resolve `ref` with gix on the mounted repository to a
single commit object. Accept a full object id (SHA-1 or SHA-256 hex) or a
ref name; peel annotated tags to the commit. Reject abbreviated object ids,
missing refs, and non-commit tips. Record the resolved commit in
`dev.pichi.build.commit`, with `sha1:` or `sha256:` in the value. Optional
`location` becomes `dev.pichi.build.location`; omit that annotation when absent.
The location and fixed mount path are not authenticated identities. Do not
record the host `ref` string separately. See [§11.1](#111-image-manifest-annotation-contract).

**Wire decoding / failure rules (v0):**

- Maps use text-string keys, definite-length CBOR, and only the fields in the
  tables above. Reject duplicate keys, noncanonical encodings that the decoder
  does not normalize, unknown fields, and wrong types.
- Decode exactly one top-level response map; after that map, any leftover bytes
  already in the read buffer are trailing data and fail the build.
- Require `v == 1` before interpreting other fields; version mismatch fails.
- When `ok` is `false`, only `v`, `ok`, and `error` may appear; success-only
  fields must not appear. When `ok` is `true`, `error` must not appear.
- Missing build-port device, undecodable CBOR, truncated response, or read
  timeout (fixed internal deadline) fails the build. There is no silent default
  plan/lock/`ref`.

**Output mount:** `/run/pichi/output` is an empty writable virtiofs mount supplied
by the host (not a path Conglobate creates). Conglobate stages privately in a
temporary child directory on that mount, then publishes by moving each top-level
layout entry (`blobs/`, `oci-layout`, `index.json`,
`build.yaml`, each flat `NNNN.cow` / `NNNN.verity` / `boot.pmi`, and any
other exported roots) into the mount root with no-replace semantics (refuse if
any destination name already exists). Fail if the mount is missing, not a
directory, or non-empty before publication. This is distinct from CLI `--output`,
which names a new directory that must not already exist (see [§11.2](#112-finalization-and-publication)
finalization / publication and [§14.2](#142-resolve-and-verify-all-supplied-inputs-before-execution)).

## 2. Standalone CLI

The standalone CLI exercises the same engine without a VM. It is the path used
by rootless CI and local tests. It does not claim the appliance image's
formatter-tool binding or build-port attestation; those are appliance-only.
Appliance production builds use the CBOR
contract in [§1.5](#15-appliance-build-port-cbor) instead of these host flags for
request transport; both paths share plan/lock/cache semantics.

Required logical inputs for the CLI are a local Git repository path, a full
commit identity (or a ref the CLI resolves locally), a context path within that
commit, context-relative plan and lock paths, a read-only image cache, and an
output directory:

```sh
conglobate build \
  --repository <local-path> \
  --commit <full-object-id> \
  --context <path-in-commit> \
  --plan <context-relative-yaml-path> \
  --lock <context-relative-lock-path> \
  --cache <read-only-image-cache> \
  --output <directory>
```

Defaults and failure behavior:

- `--context` defaults to `.`. It cannot escape the selected commit.
  Appliance CBOR uses `context` for the same role.
- `--plan` and `--lock` are required, with no default filenames or discovery.
  Both paths are relative to the selected context, not the current directory or
  each other's parent. Resolve them through the committed-context path rules,
  including confined symlink handling. Absolute paths, escapes, missing files
  and non-file targets fail. Both inputs must come from the selected commit.
  The manifest records their selected paths; the commit binds their bytes.
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
  host-supplied mke2fs (or other formatter) manifest as a TEE input. Plan
  authors cannot override formatter selection. The internal formatter profile
  and filesystem specification control formatting; no execution receipt is
  emitted.
- `--uid-map PARENT_START:COUNT` and `--gid-map PARENT_START:COUNT` remain
  available on the standalone CLI for VM-free and fixture execution of stages
  that need mappings. In the **appliance image** these ranges are hardcodes,
  not CBOR fields and not host CLI flags. Each maps image IDs starting at zero
  onto a contiguous parent range. Parent ID zero, empty ranges, overflow and
  the invalid all-ones ID are rejected. UID and GID counts may differ. All
  stages use the same pair. Images using IDs outside those ranges are
  unsupported; no silent ownership rewriting. Pass-through builds do not need
  mappings.
- `--network-namespace PATH` and `--resolv-conf PATH` remain available on the
  standalone CLI when any run step enables networking (same capture/join rules
  as before). In the **appliance image** the namespace path and resolver file
  are hardcodes, not CBOR fields. Open and retain the namespace descriptor once
  before execution; do not reopen the pathname per step. Capture resolver
  contents once and inject them temporarily, read-only, following symlinks
  confined to the command root. No implicit resolver or DNS relay. Resolver
  contents and their digest are not recorded as build annotations.
- Successful output is published only when all components and manifest
  annotations are complete. Refuse an existing output destination rather than
  overwrite it.
- Logs go to stderr; the process exit status distinguishes success from
  failure. A precise exit-code taxonomy is deferred.
- No successful artifact is published for a failed build. Failure diagnostics
  may be retained separately and do not constitute a successful image.

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

1. **Plan and lock** are loaded **immediately** from the Git object store and
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
selected commit (after ref resolution when applicable) with its algorithm in
`dev.pichi.build.commit`, and the context path in `dev.pichi.build.context`.
There is no separate tree or object-format annotation. Paths and the caller's
working tree do not substitute for object verification. Preserve committed bytes,
executable bits and symlink targets. Do not execute caller-provided hooks or
checkout filters.

The commit annotation value identifies Git's hash algorithm. Whether SHA-1 Git
identities require an additional independently authenticated commitment for the
intended threat model remains a security-design question. Hashing received
bytes alone does not establish the author's intended identity.

## 4. Image locks

The lock is a separate document, selected by `--lock`. It is not embedded
in the plan. It maps each `Remote` reference to one immutable OCI index
digest. `Local` references name tasks ([§6.1](#61-references)) and are not
entries here. `image:` references are `Local` only, so this document does not
pin them.

A `Remote` value is a struct of OCI components and must be fully qualified:
the registry host is required. Bare Hub short names are rejected. A reference
is not `Remote` merely because it contains `/`. Parsing must not let a task
name silently win over an OCI reference, or the reverse.

Proposed wire shape:

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
platform manifest, and verifies it. The carapace / filesystem root is derived
from the scute graph (exactly one tip root), not from an OCI tip annotation.
`dev.pichi.carapace.verity.hash` is never written, never read, and is stripped
on rewrite if present; other verity parameter annotations remain. Guest boot
(`corium=` / `$CARAPACE`) uses that derived measurement. Conglobate verifies
content and chain consistency against those identities. The committed lock
pins the index, which in turn binds the selected manifest; do not mirror those
identities into build annotations.

Missing pins, missing cached content, incompatible or ambiguous platform
selection, and mismatches are errors. Mutable cache tags are never consulted
to override a pin. An image without a carapace cannot supply an execution
carapace. A build never rewrites the lock; lock creation and cache population
belong to the caller.

One first-smoke exception is `tests/distro/context/lock.yaml`. That index's
manifest descriptor is a harness placeholder: the digest is 32 bytes of
`0x11` and the declared size is 2. The host stores the real carapace manifest
at that blob path. Those bytes need not be 2 long and need not hash to the
placeholder. The guest still refuses a read over the small-metadata limit,
records the SHA-256 of the bytes it captured, and verifies every blob that
manifest names. Any other descriptor keeps the size and digest checks.

## 5. Context and pipeline

The context has no required configuration directory or filenames. For example:

```text
<committed-context>/
  plans/image.yaml
  locks/images.lock
  config/
  sources/
```

Select those files with `--plan plans/image.yaml --lock locks/images.lock`.
These names are examples, not defaults. The lock path selects a separate
document. The plan does not embed the lock. Context paths stay relative to
the context root, even when the plan lives in a nested directory.

The approved plan is a set of named tasks plus one `image:` assembly. It
replaces the fixed pipeline `bootstrap` → `carapace` → `firmware` →
`transform` as the plan source of truth. Those names are not top-level
section keys. `platform` is not a task kind. There is no `kind: import`.

A task is a kind-tagged enum. The kinds are `bootstrap`, `extend`, and
`build`. Reject duplicate keys, unknown keys, and unknown kinds rather than
merging or ignoring them. YAML null is not an alias for omission or emptiness.
An empty plan cannot produce an image.

### 5.1 Tasks

| Kind | Carapace input (`from:`) | Retained output |
| --- | --- | --- |
| `bootstrap` | Tool carapace | A new carapace. The engine formats `filesystem:` outside the sandbox. `exec` only populates `/mnt`. |
| `extend` | The carapace to extend | A carapace. The engine always rewrites the DTB for the new root hash and re-exports the PMI when one is present. Boot rides with the task. |
| `build` | Tool carapace | Artifacts only (`into:`). No carapace output. |

`build` replaces the earlier names `export`, `make`, and `produce`. Those
names are not kinds.

Custom PMI/DTB bytes exist only when an explicit task exports boot. That work
is not a fourth kind and not an implicit pipeline stage. `build` yields
artifacts and no carapace, which is the kind available for a producer that
exports boot and no carapace. Extend exports boot when it re-exports a PMI.
Automatic DTB rewrite is not a task and not a per-plan job. See
[§8](#8-dtb-transformations).

A tool carapace does not become output ancestry because commands execute
inside it.

### 5.2 Image assembly

Producing a pichi image is not a task. An image is the assembly of one
carapace and optional boot (PMI and DTB). The image manifest references those
components. Launch configuration and any component beyond carapace and boot
remain unspecified ([§13](#13-remaining-decisions)).

`image:` references are `Local` only. They name a task in this plan.
Two forms are valid.

Short form: one reference. Assembly uses whatever that single task already
exports: carapace, boot, or both. The task must export a carapace or boot or
both. Exporting neither is invalid. The short form does not mean
carapace-only.

```yaml
image: rootfs
```

Long form: `root` and `boot` name tasks. Each reference must provide that
export.

```yaml
image:
  root: rootfs
  boot: firmware
```

`root` requires a carapace export. `boot` requires a boot export.

### 5.3 Closure

Only tasks on the image carapace/boot closure run. A task is in the closure
when the `image:` carapace reference, the `image:` boot reference, or a task
already in the closure reaches it through `from:` or `copy`. Orphan tasks do
not run. YAML key order does not select the closure and does not order
execution. A task runs after the tasks that produce the inputs it references.

## 6. Execution plans

The task vocabulary is `kind`, `from:`, `exec`, `into:`, `copy`, and, for
bootstrap, `filesystem:`. Examples show locked fields. Unknown fields and
unsupported combinations are errors.

The lock does not specify which YAML key nests the task map. Each task has a
name that a `Local` reference cites.

### 6.1 References

A reference is an untagged enum: `Local` | `Remote`. References have no `./`
prefix.

`Local` is a limited-charset newtype and names a task in this plan. This
lock does not enumerate the character set. `Remote` is a struct of OCI
components and must be fully qualified: a registry host is required. Bare Hub
short names are rejected.

A value is not `Remote` because it contains `/`. That test is rejected. A
task name must not silently win over an OCI reference, and an OCI reference
must not silently win over a task name. The variants have to be unambiguous
without those heuristics.

`from:` accepts `Local` or `Remote`. `image:` accepts `Local` only.
`copy.from` is an artifact name in the flat namespace of
[§6.3](#63-exec-from-into-and-copy), not this reference enum.

The lock document ([§4](#4-image-locks)) is separate. It pins `Remote`
references to immutable OCI index digests. Local task names are not entries
in that document.

### 6.2 Task kinds

`bootstrap` requires `from:` (the tool carapace) and `filesystem:`. The engine
formats that filesystem outside the command sandbox, on a new zero-backed
carapace layer, before `exec` starts. The plan does not run mkfs or mount.
`exec` populates `/mnt` only.

Commands see two writable mounts. `/` is the tool carapace with a disposable
writable layer. Authors may install tools there; those changes persist across
the task's steps and are discarded when the task completes. `/mnt` is the new
output filesystem. Commands populate it with ordinary filesystem and
package-manager operations. They do not receive the block device or a mount
privilege.

`/mnt` is reserved during bootstrap. Resolve its mountpoint within the pinned
tool root and never follow an image symlink outside that root. Paths under
`/mnt` address the new filesystem. Other absolute paths address the tool root.
The context is never mounted into commands.

No tool scutes and no tool boot components are inherited. A deliberate `copy`
can still contribute output bytes. After writers stop and both filesystems are
cleanly unmounted, seal the output scute and package its carapace. There is no
image import and no image-to-scute conversion. The first tool carapace is
produced outside this plan.

`extend` requires `from:` (the carapace to extend) and its commands in `exec`.
Existing scutes pass through. The task adds one new scute. Every `exec` step
modifies that same writable snapshot. Each command keeps its own isolation,
timeout, and network selection. Stop and reap its descendants, flush, and
unmount before the next step. Seal the shared snapshot only after every step
succeeds. A failed task produces no publishable layer. No commands means no
new scute. Skipping empty block deltas is not part of this model. Sealing is
not filesystem compaction.

The task always rewrites the DTB for the new carapace root hash and re-exports
the PMI when the input has one. Boot rides with the extended task. Do not
replace only half of the PMI/DTB pair, and do not add a boot-discard
operation.

`build` requires `from:` (the tool carapace). It produces the artifacts named
by `into:` and does not produce a carapace. Commands use a disposable tool
root at `/`. There is no `/mnt` output mount. Extract declared artifacts
through confined handles after all command writers stop. Intermediate tool
changes do not become scutes. Only the declared artifacts survive. `export`,
`make`, and `produce` are not accepted as `kind`.

### 6.3 exec, from, into, and copy

`exec` is the ordered command list. The name replaces `derive`. `from:` is the
carapace input. `into:` declares artifact outputs. `copy` brings a cross-task
artifact into the task. There is no `artifacts:` input map.

```yaml
exec:
  - run: dnf install -y nginx
    network: true
    timeout: 5m
```

The artifact namespace has two sources. One is the git build context, which is
a tree. The other is task outputs, which Conglobate defines from `into:`.

`into:` keys and `copy.from` are flat bare names. A name of the form
`task:name`, and any other colon namespacing, is rejected.

`image:` selects carapace and boot exports. It does not select `into:` names.
Carapace and boot exports are the components described in [§5.1](#51-tasks)
and [§5.2](#52-image-assembly).

**Open, not locked.** Should `into` keys be paths? The git context preloaded
into the artifact namespace is a tree, not a flat set of names. Until that is
decided, keys stay flat bare names. The question is recorded again under
[Remaining decisions](#13-remaining-decisions). This lock does not define a
value grammar for `into:` entries and does not define a destination path on
`copy`.

### 6.4 Filesystem

`filesystem:` is a kind-tagged enum on `bootstrap`. It is not a `type:` field
plus a generic `properties` map. `kind` selects the variant. That variant's
properties are a dedicated type. Ext4 properties (`Ext4Properties`) include
`label`. `FilesystemSize` is a strict type, not a free-form string. Rust type
names for these values do not use a `Document` suffix.

```yaml
filesystem:
  kind: ext4
  size: 4GiB
  label: rootfs
```

`size` is a `FilesystemSize`. `label` is the ext4 property. A nested generic
`properties:` map is not the schema.

The size spelling is that strict type: a positive integer followed immediately
by `B`, `KiB`, `MiB`, or `GiB`, 4096-byte aligned, between 16 MiB and 64 GiB
inclusive. No fractional values and no implicit units. `label` defaults to
empty and is limited to 16 UTF-8 bytes without NUL. Unknown properties fail.
The carapace logical device remains 64 GiB. Trusted formatting uses exactly
the requested number of 4096-byte blocks and leaves the rest zero-backed. Size
does not change carapace wire geometry. Validate this before privileged
execution. Do not accept arbitrary formatter arguments, executable paths, or
shell fragments from the plan or the host. Fail if the driver, the internal
formatter map entry, or the required idmapping and metadata support is
unavailable. Ext4 is the initially validated backend. Other kernel filesystems
require qualification. Validate the effective formatting configuration and
internal formatter profile before execution; do not emit formatter receipts.

### 6.5 Command execution

Commands execute as UID/GID 0 in a mapped user namespace. An
environment-provided BPF LSM exception permits foreign SELinux labels without
command `CAP_SYS_ADMIN`. That capability is absent from every command
capability set. Commands never receive initial-user-namespace root authority.
A tool carapace only needs enough initial functionality to obtain and run
further tools.

Network access is opt-in per `run` and defaults to false. For an enabled step,
conglobate joins the supplied `--network-namespace`. For a disabled step, it
creates a fresh disconnected namespace. Commands cannot administer either
namespace. The environment contract for NAT, reply traffic, and inbound policy
is [§14.8](#148-networking-and-dns). Enabled steps may share network state,
including conntrack. No command process may survive its step.

Every `run` has a finite positive command-execution timeout. The default is
`5m`. The optional `timeout` field accepts duration strings such as `30m` or
`1h`. Zero, infinite, invalid, and overflowing values fail validation. Setup
and teardown have separate fixed internal deadlines. They are not plan or
CLI settings. Each `run` gets a fresh filesystem mount and a fresh command
sandbox.

Common execution rules:

- The committed context stays private to the builder. Commands receive
  selected bytes only through explicit `copy`.
- Commands run through `/bin/sh -eu -c`, starting at `/`. Environment and
  working-directory changes are written inside a command. They do not persist
  implicitly between shells. Processes cannot outlive a step.
- No uncontrolled inherited host environment is exposed to build commands.
- The fixed-pipeline image-path `copy` fields `owner`, `group`, and `mode`,
  and Go `filepath.Match` source patterns, are not part of this format. This
  lock does not define replacements for them.

### 6.6 Rejected Docker behavior

These rejections stay in force:

- No copy of a whole stage root.
- No implicit promotion of a tool carapace into output ancestry.
- No `ADD` of URLs.
- Finalize an artifact before another task takes it with `copy`.
- Artifact names are not a security boundary.
- A task does not inherit every intermediate output.
- No Docker layer cache follows from this format.
- No last-stage-wins rule. `image:` and the closure select outputs.

## 7. Build variables

`CARAPACE` identifies the output carapace root as plain hex once `bootstrap`
or `extend` has produced it, or once `image:` has selected it. It is absent
when the closure has no output carapace.

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

Plans have no `transform` section. The encodings below are the engine's
DTB edit vocabulary, applied by the automatic rewrite in
[§8.3](#83-automatic-rewrite). Authors do not supply this list. Each operation
has exactly one `op` discriminator. `set` also has a flat `type` discriminator
selecting the encoding of `data`. `property` names an absolute node path
followed by the property name; for example, `/chosen/bootargs` selects
`bootargs` on the `/chosen` node.

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
or absent DTB is an error for the operation.

**Replace every match; zero matches leave the string unchanged.** There are
no match-count settings. Target validation happens before matching: an invalid
string is an error even if the expression would have no matches.
Invalid regex syntax or invalid replacement-template syntax fails validation.

| Target state | Regex operation |
| --- | --- |
| DTB, node, or property missing | Fail |
| Property is not a single valid string | Fail |
| Valid string, zero matches | Succeed without changing it |
| Valid string, one or more matches | Replace all matches in that operation |

Use Rust `regex` syntax and capture expansion, with the build-variable extension
described above. Do not implement sed syntax or shell out. `${1}` in the
examples preserves the whitespace consumed by the boundary capture.

### 8.3 Automatic rewrite

The automatic rewrite is engine-owned. It is not a task and not a per-plan
job. Plans have no `transform` key. Authors cannot replace the sequence,
append to it, or disable it with an empty list.

The engine applies the built-in sequence when a DTB is present and either:

- an `extend` task produces a new carapace root. The rewrite is not optional
  for that task. The task also re-exports the PMI when one is present, so
  boot rides with the task; or
- image assembly binds an image root to imported boot lineage.

When no DTB exists, skip the rewrite. Skipping is not a plan switch.

The built-in sequence is:

```yaml
- op: regex
  if:
    exists: /chosen/bootargs
  property: /chosen/bootargs
  match: '(^|\s)corium=[^\s]*'
  replace: '${1}corium=${CARAPACE}'
```

It refreshes an existing `corium=` argument and never introduces one. The
task that exports boot establishes the initial binding. A matched binding with
undefined `CARAPACE` fails. Non-string command-line data remains an error.
This generic regex does not validate all corium arguments or repair duplicate
bindings.

Within an existing DTB, the sequence uses `if.exists` to skip an absent
command line. No operation reads a missing property successfully, and a
missing DTB never makes an existence condition return false.

The finalized DTB in the image manifest binds the rewrite result. Do not emit
transform execution records; see [§11.1](#111-image-manifest-annotation-contract).

Property operations only are in scope initially; `set` may create ancestor
nodes, but there are no explicit whole-node creation or removal operations.
Custom PMI/DTB bytes that differ from this sequence come from an explicit
task's boot export, not from a plan-authored list of these operations.

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
assembly, writable snapshots, image metadata, and publication. Immutable carapace
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
and confer no authority in the build environment. Before plan admission,
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
there is no built-in DNS relay, resolver annotation or persistent injection.

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

Firmware production precedes final DTB transformations. The final image manifest
must identify the transformed DTB, and any measurement or
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
signing contracts must export the final DTB from an explicit task. The
plan has no `transform` section and no switch that disables the automatic
rewrite. A firmware contract that rejects a changed DTB still rejects the
build when that rewrite would change the DTB.

## 11. Artifact and build annotations

Nathaniel's decision of 28 October 2026, tracked in
[Conglobate #25](https://github.com/pichi-vm/conglobate/issues/25) and
[Pichi #27](https://github.com/pichi-vm/pichi/issues/27), replaces the inventory
with build annotations on the **image manifest**. Drop `inventory.cbor`
entirely; it is not an optional output. Do not emit a replacement receipt file.

The output directory is itself a self-contained OCI image layout:

```text
result/
  oci-layout         # OCI layout version; external OCI formats remain JSON
  index.json         # output image index
  blobs/sha256/      # all referenced manifest, configuration and layer blobs
  build.yaml         # transitional BuildOutput; scutes may be empty
  0000.cow           # delta scutes only, chain order; names come from build.yaml
  0000.verity
  boot.pmi           # only when this build produced firmware
```

OCI consumers use `result/` directly. Keep the transitional `build.yaml` and
flat blobs until [Conglobate #24](https://github.com/pichi-vm/conglobate/issues/24),
after Pichi OCI import. They serve the current host packaging path: filenames
are relative to `result/`, chain order is base to top, and only delta scutes
are listed. The host prepends the operator source carapace named by `from:`.
An empty delta list is valid. This compatibility output is not a build receipt.

Include every blob referenced by the output image, including inherited blobs
from the cache. Import must not require the original cache. The final manifest
describes all components and the final transformed DTB.

### 11.1 Image-manifest annotation contract

Write these build binding facts on the published image manifest itself,
not only on an index, index descriptor, or configuration blob:

| Annotation | Value |
| --- | --- |
| `dev.pichi.build.commit` | Required: `sha1:<hex>` or `sha256:<hex>`, using the full resolved Git commit identity |
| `dev.pichi.build.context` | Required: path inside that commit; `.` selects the commit root |
| `dev.pichi.build.plan` | Required: plan path relative to the selected context |
| `dev.pichi.build.lock` | Required: lock path relative to the selected context |
| `dev.pichi.build.location` | Optional: provenance hint for where to obtain the repository |

Location says where to get it; commit says what it is; context selects a path
in that commit; plan and lock select paths relative to context. The commit
value carries its algorithm. There are no separate tree or object-format keys.
Plan and lock bytes are already bound through the commit and selected paths;
do not mirror their content digests as additional build annotations.

Location is only a locator, not an authenticated identity. Omit it when no
provenance hint is supplied; the appliance's fixed repository mount is not an
identity. Locators must not expose embedded credentials. The host-requested
`ref` is resolved before publication and is not a separate annotation.

Do not add `builder`, `inputs`, `execution`, or `outputs` mirrors. The plan,
lock, commit and OCI graph already carry the binding facts. Execution receipts,
including formatter, transform, network-step and ID-mapping records, are not
recorded in a sidecar or mirrored into annotations. This decision does not
remove input verification or execution/finalization checks.

The carapace tip root remains deterministically implied by the manifest's
scute graph. Never stamp or require `dev.pichi.carapace.verity.hash`. Keep the
existing carapace verity parameter annotations and per-scute salt/data-blocks
annotations. Boot binding uses the root derived from that graph.

### 11.2 Finalization and publication

Validate the annotation values against the selected commit, context, plan and
lock, then validate the finalized manifest and every referenced blob before
publication. Hash finalized bytes after all manifest annotations and DTB edits
are complete. Publication differs by destination:

- **CLI `--output`:** stage under a temporary sibling of the named destination,
  then publish the completed directory with an atomic no-replace rename onto a
  path that must not already exist, even if empty.
- **Appliance `/run/pichi/output`:** the virtiofs mount root already exists and
  must be empty at start. Complete every fallible check while staging privately
  in a temporary child directory on that same mount. Only then publish by moving
  each top-level layout entry (`oci-layout`, `index.json`, `blobs/`,
  `build.yaml`, flat `NNNN.cow` / `NNNN.verity` / `boot.pmi`,
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

Later attestation binds the finalized **image digest** and the **TEE/builder**.
It does not introduce a second receipt file. Measuring the builder executable
alone does not identify the kernel, libraries, labeling policy, formatter
profiles or other privileged environment that determined the result.

Standalone execution publishes the same annotated image without claiming
attestation. Report-data encoding, endorsements, verifier policy, freshness,
and hardware-specific mechanisms remain appliance integration work.

Hash the finalized bytes the trusted builder actually produced before relying
on an untrusted output transport. Re-reading mutable host output and treating
it as authoritative would break that binding. A consumer verifies retrieved
manifest bytes against the attested image digest, then verifies its OCI graph.

Attestation does not establish that plans are benign, network dependencies are
reproducible, or the author intended the pinned inputs without a separate
authenticity policy.

## 12. Validation scope

The implementation follows the ordered sandbox contract. Maintained tests
cover committed inputs, locks, typed DTB operations, pipeline combinations,
metadata, legacy inventory, publication and disposable-VM lifecycle failures.
The follow-up code tip must replace inventory checks with manifest annotation
checks. Those tests exercise the task model in §5–§6 alongside the existing
storage, isolation, metadata and publication behavior. Distro plans exercise Fedora
DNF/SELinux, Debian APT/systemd, Alpine APK/OpenRC and NixOS closure construction.
A separate firmware plan constructs an initramfs and arma PMI/DTB, then boots it.

CI checks native amd64/Arm64 Rust and guest lifecycle behavior. Arm64 hosted guest
tests explicitly record TCG when KVM is unavailable. This does not propose TCG as
the production sandbox. Confidential-computing hardware qualification is deferred
by user decision; attestation remains separate work (build-port transport is
locked in [§1.4](#14-build-appliance)–[§1.5](#15-appliance-build-port-cbor)).

The binary remains standalone and does not create VMs; privileged tests run only
inside disposable VMs. Distro plans do not imply support for every distribution
and architecture combination. The supported labeling environment disables build-VM
SELinux; an enforcing build VM requires separate qualification. Boot checks stop
at their success markers and do not qualify guest-initiated shutdown.

## 13. Remaining decisions

The design uses gix: plan and lock verify immediately; other context paths
verify on use. Plan symlinks resolve only within the committed context; hooks
and checkout filters do not execute. The lock format, image-manifest annotation
contract, YAML values, device exposure, and supported firmware measurement
contracts are specified above. Temporary writable mappings use
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
regex match-count options, and whole-node operations. The plan format has no
`transform` section and no `platform` task kind. Caller working-tree policy is
outside this interface.

### Open plan question

Whether `into` keys should be paths is not locked. The git build context
preloaded into the artifact namespace is a tree, not a flat set of names.
Until a decision, [§6.3](#63-exec-from-into-and-copy) keeps flat bare names for
`into:` keys and `copy.from`, with no `task:name` colon namespacing.

### Proposed implementation acceptance

These checks were proposed with the plan locks. They are not additional
design locks, and this document does not implement them.

1. Serde types and validation match §5–§6, with tests for short and long
   `image:`, `Local` / `Remote` parsing, kind tagging, closure-only execution,
   and the `Filesystem` enum.
2. The engine runs the `bootstrap` / `extend` / `build` closure selected by
   `image:`, applies the automatic DTB rewrite on assemble, and keeps
   transitional `build.yaml` dual-write working for smoke until #24, after
   Pichi OCI import.
3. Annotation checks replace inventory assertions: cover the exact §11.1
   keys, commit algorithm prefixes, context-relative paths, absent optional
   location, no sidecar, and unchanged verity annotations. Retain dual-write
   checks until #24 and verify annotations survive OCI copy/import.
4. An appliance smoke with a multi-task plan (at least bootstrap plus build,
   or bootstrap plus extend) passes without a registry publish, before the
   implementation change.
5. That implementation change waits until this plan text is on `main`, then
   merges by rebase after green CI and review clearance. Republish `:0.2.0`
   only if the appliance contract or digest changes.

## 14. Sandbox lifecycle

The build proceeds in this order; each failure aborts publication:

1. Validate the request, resolve the commit, load plan and lock from the
   object store, and resolve and verify every required local image input.
2. Capture external network inputs if needed. Allocate the protected RAM
   workspace and execution runtime only when the closure requires execution.
3. Execute the `image:` closure. Bootstrap formats a new snapshot over
   dm-zero outside the sandbox and populates `/mnt`. Extend produces one
   scute for its commands, rewrites the DTB for the new root hash, and
   re-exports the PMI when present. Build extracts artifacts only and
   produces no carapace. Orphan tasks do not run.
4. For each `exec` step, mount assigned roots. `copy` uses lazy verify-on-use
   context bytes; `run` constructs a fresh sandbox, executes within its timeout,
   and stops and reaps all descendants. Remove temporary mounts, flush and
   unmount before sealing task output or releasing storage.
5. Close the execution runtime. Apply the automatic DTB rewrite, assemble one
   carapace and optional boot (assembly is not a task), write the transitional
   flat `build.yaml` (delta scutes only), annotate the image manifest, and
   verify staged output.
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
   stages execute. It verifies inputs, interprets plans, assembles storage,
   owns execution and cleanup, finalizes images, and annotates the manifest. The
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
   alter supervisor state, inputs, image metadata, control interfaces, output
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
   appliance `ref` → commit on the mounted repo, context path,
   read-only image cache, explicit context-relative plan and lock paths,
   and a new output destination for the CLI (`--output` must not already exist),
   or the empty appliance mount at `/run/pichi/output` into which layout entries
   are published (see [§1.5](#15-appliance-build-port-cbor)). Neither plan nor
   lock has a default filename. Require
   committed files resolved within the selected context; reject absolute
   paths, escapes and missing/non-file inputs. Record selected paths and
   bind their exact bytes through the commit annotation. Load plan and lock
   immediately from the object store; resolve COPY and other context paths with lazy
   gix verify-on-use. Do not clone, do not treat a working-tree checkout as
   SoT, and do not eagerly capture the entire context tree. Commands never
   receive the context. Use the Git, lock, and host-architecture rules in
   DESIGN.md. Working-tree policy for callers is outside this interface.

1. Resolve plan references only through the committed, architecture-aware
   lock, pinning the top-level multi-architecture index and verifying its
   referenced content. Missing artifacts or blobs are errors. Conglobate
   does not fetch Git content, resolve tags remotely, or download extra
   image artifacts. Plan commands explicitly granted networking may
   download packages; this is distinct from input resolution.

1. Parse one YAML plan and the separate lock before execution. Tasks are
   a kind-tagged enum: `bootstrap`, `extend`, or `build`. Reject unknown
   kinds, including `kind: import`, a `transform` section, and a `platform`
   kind. `image:` is `Local` only. Run only the carapace/boot closure.
   Key order never selects that closure. References are an untagged
   `Local` | `Remote` enum. Do not classify `Remote` by the presence of `/`.
   Do not let a task name silently win over an OCI reference, or the reverse.
   `Remote` values must be fully qualified. `Filesystem` is a kind-tagged
   enum with type-specific properties and a strict `FilesystemSize`.
   Reject duplicate or unknown keys, invalid types, and null values.
   Check the filesystem backend and runtime prerequisites, required image
   exports, finite command timeouts, variable availability when evaluated,
   and DNS configuration for network-enabled commands. Reject unsupported
   behavior rather than silently weakening the sandbox.

1. Require and capture --network-namespace when any step enables networking
   as specified in section 14.8. Read --resolv-conf PATH once if networking is needed and retain the
   selected bytes privately. The argument is mandatory if any run step
   has network: true. An unreadable or invalid selected file is an error;
   there is no implicit host-file or public-resolver fallback. Resolver
   configuration and its digest are not recorded as build annotations.
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
   swap, or tmpfs-size policy to plans. Allocation/write failures fail
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
   service accounts and other workloads. Use the same effective pair
   consistently across all executing stages; do not emit an execution record.
   Retain normal kernel ownership, mode and ACL permission checking. Use nosuid,nodev;
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

1. Validate the required labeling behavior before admitting plan commands
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

### 14.5 Execute the image closure under the task rules

1. `image:` selects the closure, as specified in [§5.3](#53-closure). Run
   only tasks reachable from its carapace and boot references. Orphan tasks
   do not run. YAML key order does not choose the closure or the execution
   order. A task runs after the inputs it references have been produced.
   Producing the image is assembly, not a task.

1. A `bootstrap` task selects a tool carapace with `from:` and declares the
   output filesystem as a kind-tagged `filesystem:` value: a strict
   `FilesystemSize` and type-specific properties, for example ext4 `label`.
   Trusted formatting tools create that filesystem on the new zero-backed
   writable carapace layer before commands start. The plan does not run
   mkfs or mount. Never pass plan-provided shell commands or formatter
   arguments to privileged tooling. Validate properties against the selected
   backend. Validate the effective configuration and tool identity before
   execution. Do not emit a formatter receipt.

   Commands see the disposable writable tool carapace at / and the fresh
   output filesystem at /mnt. They can install tools into / and populate
   /mnt, for example using dnf --installroot=/mnt. Selected bytes land on
   those assigned mounts. Constrain /mnt setup against the pinned root;
   it is a reserved output mount during bootstrap. Tool modifications persist
   between its steps but are discarded when the task completes. No tool layers
   or tool boot components are inherited. Deliberately copied bytes can
   contribute to the output. Stop writers and cleanly unmount both views;
   seal the output layer directly, without import or image conversion.

1. An `extend` task takes a carapace (`from:`) and commands and produces a
   carapace. It always rewrites the DTB for the new root hash and re-exports
   the PMI when one is present. Boot rides with the task. All `exec` steps
   share one writable snapshot of the input carapace. Seal that snapshot once
   after every step succeeds. A failed task produces no publishable layer.
   Do not replace only half of the PMI/DTB pair, and do not add a boot-discard
   operation. A tool carapace does not become output ancestry.

1. A `build` task takes a tool carapace and produces artifacts only. It does
   not produce a carapace. `into:` names those artifacts with flat bare names.
   Commands use a disposable tool root at /, without a special /mnt output
   mount. Extract declared artifacts through confined handles only after all
   writers stop. Intermediate tool changes do not become scutes. There is no
   `export`, `make`, or `produce` task kind.

1. The automatic DTB rewrite is not a plan section and not a task. `extend`
   performs it for the new root hash. Image assembly performs it when the
   image root is bound to imported boot lineage. The built-in sequence is
   [§8.3](#83-automatic-rewrite). There is no `transform` key. Custom PMI/DTB
   bytes come from an explicit task. Reject a `platform` kind. Regex replaces
   all matches, leaves zero matches unchanged, and fails on a non-string.
   Missing reads fail unless guarded by property existence. `set` can create
   nodes. `remove` of an absent property fails. A missing DTB skips the
   automatic rewrite. The corium edit tolerates absent bootargs and edits
   existing `corium=` arguments, without assuming every firmware is Linux.

1. Expose `$CARAPACE` as the output carapace root once that carapace exists,
   not a tool-carapace root and not a build task's tool root. Referencing it
   when unavailable fails. Expand string values with the documented dollar
   escaping.

### 14.6 Keep COPY and output extraction controlled

1. Commands never receive a context mount. `copy` selects a flat bare name
   from the git context tree or from a task output, after that source is
   finalized. It does not copy a whole stage root and it does not fetch a
   URL. The supervisor places selected bytes into an assigned filesystem
   through its kernel mount, while command writers are stopped. Use scoped
   handles and root-confined path resolution. Symlinks and metadata must not
   redirect supervisor operations outside the target.

1. `copy` is a supervisor-owned operation, not an image-authored script with
   context access. Use a controlled mount session without command writers,
   root-confined path resolution and narrowly scoped handles. Complete it
   before the next run. Apply the same rules to disposable tool roots and
   bootstrap output roots; do not expose the context object store to commands.

1. Extract declared `into:` artifacts, and boot files a task exports, only
   after commands and descendants have stopped. Validate types, bounds,
   paths, and formats.
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

1. Every run has a finite positive timeout. Default: 5 minutes. Plans
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
   tested as implementation constants. They are not plan or CLI knobs
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

1. Emit a self-contained OCI image layout with the build annotations in
   [§11.1](#111-image-manifest-annotation-contract). Include every referenced
   output blob, including inherited ones. Do not emit an inventory or mirror
   builder, input, execution or output records into annotations. The commit,
   context and plan/lock paths bind committed build inputs; network acquisition
   is not claimed to be reproducible or exhaustive input accounting.

1. Require a new output directory. Stage the complete artifact, verify its
   identities, and publish atomically without replacement. Failures emit
   diagnostics, not a successful artifact. Cache mutation/import and
   attestation belong to external integrations. The appliance may attest
   the finalized image digest and TEE/builder later.

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
Plan and lock load immediately from the object store; COPY and other context
paths use lazy gix verify-on-use without exposing a context mount. Locked
multi-architecture indexes select only the host architecture. Image components
and verified chain roots remain separate from live device mappings.

The engine runs the tasks on the `image:` closure: `bootstrap`, `extend`,
and `build`. It owns one RAM workspace and execution runtime, borrowed by
those tasks. Bootstrap formats its new writable snapshot over dm-zero
**outside the command sandbox** and uses separate tool and output mounts.
Extend's commands share one writable snapshot and produce one scute. Build
uses a disposable tool carapace and extracts `into:` artifacts only. Tool
inputs are bound by the committed plan and lock; they do not become output
ancestry.
Orphan tasks do not run. After the closure, the automatic DTB rewrite and
image assembly run. Assembly is not a task. The binary sequences this closure using immutable task results and activates
local carapace chains only when a dependent task needs their filesystem.

`StageOutput` holds results without command/mount authority. Runtime cleanup
precedes DTB transformation, image assembly, manifest annotation and
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

Each command reconstructs mounts/isolation and has a finite plan timeout
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
