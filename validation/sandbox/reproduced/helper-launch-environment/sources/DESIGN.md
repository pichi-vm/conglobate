# Conglobate image build design

Status: design draft for implementation. Requirements use “must” or direct
statements of behavior. Candidate interfaces are labeled “proposed”; unresolved
details are collected in [Remaining decisions](#13-remaining-decisions).
This document specifies the target design, not the current implementation.

Conglobate builds images from committed source and immutable image inputs. It
can create a new carapace using a tool carapace, extend an existing image, build
replacement firmware, and transform the final device tree. It emits an
importable image and an inventory binding its supplied inputs to its outputs.

## Trust model — required for attested builds

**The supervisor and filesystem worker are trusted. Build commands are
untrusted. The build kernel is trusted.** Running without host root is useful,
but does not establish this boundary.

The trusted builder verifies inputs, interprets recipes, assembles storage,
enforces execution policy, finalizes outputs, and generates the inventory.
The filesystem worker belongs to that same trusted computing base: it may
share the supervisor's trust domain. A separate worker security domain is
not required. Its implementation must safely handle hostile filesystem
requests and admitted image contents.

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
descendants, revoke their write access, and finish the trusted worker's writes.
The bytes hashed for the inventory must remain stable through publication.
Future attestation must bind the trusted execution environment, including the
filesystem implementation, to that inventory; measuring conglobate alone is
insufficient.

This is the agreed target model, not a claim about the existing executor.
The namespace/FUSE prototype demonstrates selected compatibility and isolation
properties; adversarial validation and production integration remain required.

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
   subdirectory, and a read-only pichi-style image cache, it produces an
   importable artifact. The current storage backend requires root; fully
   unprivileged storage assembly is under investigation. Commands never
   receive that initial-namespace root authority in the target design.
   Conglobate does not require a VM, virtiofs, GPT
   transport, PID 1, or a particular hypervisor.
2. **The official build appliance** wraps that same binary in a bootable pichi
   image. Its wrapper initializes the guest, exposes the supplied inputs,
   invokes conglobate, exports results and optional attestation evidence, and
   shuts down. Transport and boot lifecycle belong to the wrapper.
3. **`pichi build`** implements the appliance contract. It prepares committed
   inputs and cached images, supplies resources, launches the appliance, and
   retrieves/imports the result. It does not interpret private intermediate
   files to reconstruct the output image itself.

The primary conglobate integration tests run without VM infrastructure.
Appliance boot tests verify the wrapper and transport separately.

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
a subdirectory within that commit, and a read-only image cache. Proposed CLI:

```sh
conglobate build \
  --repository <local-path> \
  --commit <full-object-id> \
  --context <relative-subdirectory> \
  --cache <read-only-image-cache> \
  --output <directory>
```

Proposed defaults and failure behavior:

- `--context` defaults to `.`. It cannot escape the checkout.
- `--commit` identifies a commit, not a branch, tag, abbreviated hash, or index
  snapshot. Locator and identity are separate inputs.
- The cache is never mutated. Missing locked objects are errors; image tags
  are never resolved opportunistically during a build.
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

It verifies the selected Git objects and makes a private read-only checkout of
the commit. The recipes, lock, scripts, and copy sources all come from that
checkout. Subsequent changes to the caller's working tree cannot affect it.
Submodules and Git LFS are rejected in the initial implementation.

### 3.1 Context materialization

Use ordinary Git checkout behavior, without an additional metadata-normalization
policy. The context is not converted to a carapace. It remains private and
read-only during execution. Record the repository's object format, selected
commit, and context subtree identity in the inventory.

Do not run caller-provided hooks or arbitrary checkout filters as an implicit
part of ingestion. Git backend selection must preserve the agreed checkout
semantics and reject unsupported submodule/LFS content rather than silently
building from pointer files. The precise implementation checks need tests.

Git's object format must be recorded. Whether SHA-1 Git identities require an
additional independently authenticated commitment for the intended threat
model remains a security-design question. Hashing received bytes alone does
not establish the author's intended identity.

## 4. Image locks

A committed YAML `pichi.build/refs.lock`, relative to the context directory,
maps each recipe image reference to one immutable OCI index digest. Proposed
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

The proposed context layout uses a `pichi.build/` recipe directory:

```text
<committed-context>/
  pichi.build/
    bootstrap.yaml
    carapace.yaml
    firmware.yaml
    platform.yaml       # reserved; implementation deferred
    refs.lock
    transform.yaml
  ...                   # source, configuration, scripts, patches
```

The stage filenames sort in execution order:

```text
bootstrap -> carapace -> firmware -> platform -> transform
```

`refs.lock` is an input, not a stage. Whether to retain the `pichi.build/` name
permanently is open; it does not imply dependence on the pichi CLI.

The path passed as `--context` selects `<committed-context>`, not its
`pichi.build/` child. Copy sources are relative to that same context root.

The pipeline carries an assembled image forward. Separately, a stage may use
a carapace as its execution environment. **A tool input does not become output
ancestry merely because commands execute inside it.**

### 5.1 Stage summary

| Stage | Execution input | Retained output |
| --- | --- | --- |
| Bootstrap | Required tool carapace | A separately generated filesystem imported as a new base carapace |
| Carapace | Bootstrap output, otherwise required `from` image | Inherited image with additional scutes |
| Firmware | Resulting carapace, or explicit tool carapace | Replacement PMI and DTB together |
| Platform | Tool carapace, details deferred | Transformed DTB |
| Transform | No execution carapace | Declaratively edited DTB |

Omitted stages pass existing output forward. If bootstrap is present and
carapace is omitted, firmware can use the bootstrap result directly. A carapace
recipe with no directives is also a valid pass-through when an input exists.
Without bootstrap, that recipe still needs `from`; a literally empty file
cannot select an input image.

All components of a base image propagate unless a stage replaces them.
There is no feature for explicitly discarding inherited firmware to obtain a
carapace-only derivative. A bootstrap tool image's components are not inherited
because it is not the base of the output image.

### 5.2 Output kinds and file combinations

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

For an existing image requiring only DTB edits, a `carapace.yaml` with `from`
and no directives selects and passes through the base image.

Existing launch configuration and other image components propagate with the
base. How a recipe creates or overrides those components is not yet specified;
the pipeline described here defines carapace and firmware production only.

## 6. Execution recipes

The shared vocabulary is `from`, `derive`, and `into`. A `derive` list contains
ordered `run` and `copy` directives. Unknown fields and unsupported combinations
are errors rather than ignored instructions.

Commands execute as real root in a disposable or layered working filesystem.
Tools may be installed during execution; a tool carapace only needs enough
initial functionality to obtain and run them. A Fedora image with `dnf` can
build a new Fedora image containing `dnf`, which can then serve as a future
tool carapace. The first tool carapace is bootstrapped externally.

```yaml
derive:
  - run: dnf install -y kernel-devel
    network: true
  - copy:
      from: ./source
      into: /build/source
```

Network access is opt-in per `run` directive and defaults to false. Network
policy is enforced by the execution environment, not merely by an environment
variable or package-manager setting.

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

`bootstrap.yaml` creates a separate filesystem using a tool carapace:

```yaml
from: registry.example.com/fedora-tools:43
derive:
  - run: dnf install -y mkosi
    network: true
  - copy:
      from: ./mkosi-project
      into: /build
  - run: cd /build && mkosi --output-dir=/out
    network: true
into: /out/rootfs.raw
```

The example assumes the mkosi configuration emits that filesystem filename.
`from` is required. Commands share a disposable writable filesystem. Only the
filesystem image named by `into` is imported into a new base carapace, using the
carapace planned image reader and devmap verity formatter. No tool scutes are
inherited.
The proposed initial filesystem-image contract is a bare filesystem image.
The binary does not impose an ext4-only policy: available filesystem support
belongs to the execution environment and, for VM builds, the appliance image.

### 6.2 Carapace

`carapace.yaml` retains filesystem modifications as new layers:

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

The retained model is one new scute per directive. Each directive executes
against a writable snapshot of the preceding filesystem state. Temporary mounts
and processes must be quiesced before the scute is finalized. No directives
means no new scutes. Skipping empty deltas is not part of the initial model.

### 6.3 Firmware

`firmware.yaml` replaces the PMI and DTB as a pair. It commonly invokes arma,
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

All directives share a disposable writable environment. Intermediate changes
do not become scutes. Only the declared PMI and DTB survive. When the stage is
absent, existing firmware propagates unchanged to DTB finalization.

### 6.4 Platform

`platform.yaml` is reserved for arbitrary DTB transformations executed in a
tool carapace, with the same recipe vocabulary. Implementation and the exact
working-DTB interface are deferred until there is a user requiring it.

An implementation that does not support this stage must reject its presence,
not silently ignore it. Declarative transforms do not require this stage.

## 7. Build variables

After carapace construction, `CARAPACE` identifies the final output carapace
root as plain hex. It also exists when bootstrap produced that carapace and
`carapace.yaml` was omitted.

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

`transform.yaml` is an ordered list of operations executed directly through
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

- Absent `transform.yaml`: select the built-in sequence.
- Present `transform.yaml`: use exactly the authored sequence.
- Empty file or empty sequence: perform no transformations.
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
An authored `transform.yaml` requires a DTB, including an explicitly empty
file. Within an existing DTB, the default uses `if.exists` to skip an absent
command line. No operation reads a missing property successfully, and a
missing DTB never makes an existence condition return false.

The inventory records the effective default and whether its stage or operation
was skipped. Property operations only are in scope initially; `set` may create
ancestor nodes, but there are no explicit whole-node creation/removal operations.

## 9. Privileged execution and resource ownership

The trust model above supersedes the original shared-root execution proposal.
Commands must not have root authority in the builder's initial user namespace.
The candidate design gives commands UID/GID 0 in a mapped user namespace and
access to a filesystem served by the trusted worker. The exact capability and
syscall restrictions remain under validation. Whether storage assembly can
also run entirely unprivileged is a separate implementation question.

Candidate-policy tests show that removing `CAP_SYS_ADMIN` from the owning user
namespace prevents `security.selinux` writes through the tested FUSE backend,
even with SELinux disabled in the builder. Retaining it in that user namespace,
while filtering mount and namespace-changing syscalls, restores labeling and
passes the bounded restriction and RPM transaction probes. This is not yet a
qualified policy: reachable ioctls and other interfaces still require review.
See `validation/sandbox/policy/README.txt` for evidence and limits.

Commands use private mount, PID, IPC, UTS, and network namespaces. Filesystem
service may share their owning user namespace where required for metadata
semantics, while remaining outside their PID namespace and reachable mount
tree. Every step's descendants must terminate before the next step or output
finalization. Namespace membership alone does not establish all required
isolation properties.

The worker-identity validation makes this distinction concrete. Without PID
isolation or the command filter, siblings with the same outer UID pass signal
permission checks; sharing the worker's user namespace also permits the tested
memory read and descriptor duplication. A private child PID namespace denies
the tested ancestor-worker pidfd signal query even when that handle is
deliberately supplied. These are bounded canary tests, not approval of a shared
identity configuration. Worker placement, PID isolation, descriptor closure
and command restrictions must be qualified together. See
`validation/sandbox/policy/README.txt` for the matrix and remaining limits.

The intended ownership split is:

| Resource | Owner |
| --- | --- |
| Input cache and completed artifact | Supervisor |
| Loop devices and device-mapper stack | Supervisor with explicit cleanup |
| Working mounts | Private mount namespace with private propagation |
| Recipe descendants | Execution process group/PID namespace, fully reaped |
| Per-step network attachment | Explicitly provisioned namespace/resources |
| Context | Immutable, read-only materialization |

Mount namespaces do not make device-mapper or loop-device objects private.
Names, handles, and cleanup must remain safe for concurrent builds. Access to
`/dev`, `/sys`, `/proc`, and `/tmp` needs a deliberate exposure policy; binding
the supervisor's directories wholesale is not an isolation design.

The current implementation mounts a private `/proc`, an empty read-only `/sys`, and
a private `/dev` containing null, zero, random, urandom, standard descriptor
links, and a tmpfs `/dev/shm`. Runtime mountpoints must be directories; newly
created mountpoint directories are removed before sealing a layer. Its commands
retain the execution environment's root capabilities without introducing user
namespaces. They receive only a fixed PATH, HOME, LANG, and the applicable
CARAPACE value. Helpers use a private copy of the running builder executable.
These are legacy implementation facts, not the target security contract.
Standard input is `/dev/null`; recipes do not consume unrecorded caller input.
The target exposes only deliberate stdio channels, with no builder executable
or outside-root descriptors available to commands.
Runtime device nodes must be dedicated to the command environment, with the
appropriate mapped ownership. Bind-mounting the builder's own device-node
inodes would let command-side metadata changes affect builder resources.
The permitted device numbers must still be restricted; private inodes do not
make the underlying kernel devices private.

Conglobate presumes its environment supplies upstream connectivity. Every step
has its own network namespace: `network: true` receives outbound connectivity
and replies; other steps have no external connectivity. Commands do not share
the builder's network namespace or internal control endpoints. The outbound
and DNS implementation remains under validation. Conglobate does not toggle
the host's primary NIC.

Terminate remaining recipe descendants before starting another step or
finalizing output. Clean up mounts, devices, and scratch storage on success,
error, cancellation, and recoverable startup after a crash. Do not rely on
poweroff.

Validation has demonstrated that guardian/monitor watches alone leave commands
running if both owners disappear. A candidate outer PID namespace containing
all build actors, with a trusted init process, passes an active-build init-death
test while the monitor is stopped: kernel teardown stops descendants and releases
FUSE without observer intervention. A second test passes with the filesystem
worker stopped and FUSE requests queued. A normal full-build control also passes
with init reaping adopted zombies and requiring no remaining descendants before
exit. Plain launcher death still leaves the namespace alive; an established
parent-death link passes active-build teardown in the prototype. This remains an
experimental lifetime design. Bounded safe-Rust handoff tests use an inherited
original-parent pidfd checked after arming the death signal, including a nested
PID-namespace init. UID/GID changes clear the signal; it must be re-armed and
ownership rechecked after such transitions. Ordinary exec preserves it in the
tested helper. The owning thread must remain alive: a process pidfd alone does
not catch that thread exiting before setup while other parent threads survive.
Use a dedicated persistent launcher owner rather than an arbitrary worker-pool
thread. The safe-Rust ownership helper also passes active-launcher-death tests
in the fixed build pipeline, including a stopped FUSE worker with queued requests;
normal completion through the same helper also passes, with init reaping an
adopted zombie and requiring no descendants before exit. Namespace creation,
spawning and init reaping still use trusted Python fixtures. Production
integration, remaining credential/exec transitions, final
guardian/monitor completion protocol and uninterruptible kernel-I/O cases are
not yet qualified.
See `validation/sandbox/chain-pid-lifetime/README.txt` and the retained failing
control in `validation/sandbox/chain-owners-death/README.txt`.

Writable snapshots and build scratch storage use RAM-backed storage with
ordinary tmpfs behavior. The execution environment supplies capacity and
resource limits; conglobate does not configure memory budgets, tmpfs sizing,
or swapping policy. There is no `--scratch-size` interface. The binary owns
its temporary objects, handles allocation/write failures, and cleans up after
the build. There is no disk fallback. Output artifact storage is separate from
execution scratch.

SELinux metadata preservation and the build host's enforcing policy are
separate concerns. Tests must establish real behavior for labels, capabilities,
ACLs, ownership, and xattrs. Root alone is not a specification of that behavior.

Standalone privileged recipe execution is not promised to safely contain
arbitrary hostile code. Namespace isolation and the VM/CC threat boundary must
be documented separately from convenient execution isolation.

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
disable subsequent transforms with an empty `transform.yaml`.

## 11. Artifact and inventory

The output is a self-contained OCI image-layout directory with a CBOR
inventory. Proposed enclosing layout:

```text
result/
  image.oci/          # OCI image layout; external OCI formats remain JSON
  inventory.cbor     # successful input/output binding
  evidence/          # optional evidence binding inventory bytes
```

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
version: 1
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
  architecture: amd64
  recipes:
    carapace.yaml: {algorithm: sha256, digest: <bytes>}
    firmware.yaml: {algorithm: sha256, digest: <bytes>}
  transform:
    source: builtin
    digest: {algorithm: sha256, digest: <bytes>}
    target_present: true
  network_steps:
    - {recipe: carapace.yaml, step: 0}
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
Absent output components are omitted. Recipe identifiers are relative to the
recipe directory. Input roles refer to use, not output ancestry. Source locators
are provenance hints, not authenticated identities, and must not expose
embedded credentials.

Inventory coverage must include effective build options, selected architecture,
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

### 11.2 Version-one inventory wire profile

The implementation uses RFC 8949 section 4.2.1 core deterministic CBOR:
shortest integer and length encodings, definite-length containers, and map keys
ordered by the bytewise lexicographic order of their deterministic encodings.
Floating-point values and CBOR tags are not part of this schema. Digests and
Git object identities are byte strings. SHA-256 digests have 32 bytes; Git
identities have 20 or 32 bytes according to `object_format`.

The required top-level fields are `version`, `builder`, `context`, `inputs`,
`execution`, and `outputs`, with the nested fields illustrated above. Version
is exactly 1. Consumers reject unknown fields, unsupported versions, duplicate
keys, noncanonical encodings, and trailing data. Absent optional output roots
and firmware digests are omitted. An input's `carapace_root` is also omitted
when a firmware-only base passes through without execution; execution-tool
inputs must have a 32-byte root. Such a base retains the `carapace-base` role,
which identifies the selecting recipe, not a requirement that scutes exist.
`outputs.scutes` and `inputs` are arrays,
including when empty. Scutes are recorded in base-to-top chain order; inputs
are sorted by reference, with unique sorted roles. Network steps occur in
pipeline and directive order.

`execution.recipes` also includes `refs.lock`. The transform record adds a
required boolean `default_skipped`, indicating that the implicit default
encountered no DTB or no bootargs property; an authored transform always records
false for this field. `target_present` records whether a DTB was supplied.
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

## 12. Validation and implementation sequence

The command-isolation redesign is not ready for implementation selection.
[SANDBOX-VALIDATION.txt](SANDBOX-VALIDATION.txt) tracks the 21 validation areas,
including unresolved tests and observed limitations. In particular, the tested
fuse2fs stack stores explicit ACLs but fails named-user access and default-ACL
inheritance probes. Metadata storage alone is not evidence of correct filesystem
semantics. The prototype also does not yet apply the final capability/syscall
policy or prove the complete writer-revocation and publication sequence.
An external experimental ACL patch passes the tested common access, chmod,
inheritance and remount cases, but its original UID-based SGID handling is
incorrect. A later paired libfuse/worker prototype propagates the kernel's ACL
SGID decision and passes both capability-sensitive cases, plus restricted
layered-output checks. This remains an experimental pair without a maintained
API/adoption path; see `validation/sandbox/acl-protocol/README.txt` and
`validation/sandbox/chain-acl/README.txt`. The earlier faulty pair remains a
negative control, not the latest candidate result.
Mapped FUSE also rejects setting immutable/append-only flags, trusted xattrs
and device-node metadata in differential tests that succeed under initial root.
Timestamp and ordinary xattr persistence do not resolve those compatibility
gaps; see `validation/sandbox/metadata/README.txt`.
Finalization fault injection also shows that default fuse2fs can log final
write errors while returning success. Successful unmount and worker exit alone
cannot authorize publication without a validated error-reporting contract.
A separate experimental worker patch now reports failure for the reproduced
final-write and close-only errors, including an earlier I/O failure followed by
device recovery. This establishes a remedy for those tested paths, not a
qualified backend or completed publication protocol; see
`validation/sandbox/finalize/README.txt`.
With a worker-opened file descriptor retained, a fresh namespace using the same
outer UID can read the tested owner-only key after the worker exits; another UID
is denied. Process reaping alone must not be the identity-recycling criterion.
Retained credentials/resources and key anchors require accounting. This does not
bypass command key-syscall denials. Allocator/worker integration remains open.

Session-keyring inheritance also survives the tested namespace combination;
the launcher must isolate that channel. The large prototype's image-Python
cleanup sequence executes image code too early and is not an acceptable launch
sequence. Its replacement now passes a read-only canary fixture and the existing
larger fixture's checks; the complete launcher policy is still unvalidated.

Recommended implementation sequence:

1. Finalize the remaining recipe/transform schema details as portable types;
   encode the inheritance rules specified here.
2. Implement the generic DTB editor and default dispatch with unit tests.
3. Specify committed-context materialization and the architecture-aware lock.
4. Build the standalone executor with safe storage APIs and explicit cleanup.
5. Package importable images and emit deterministic CBOR inventory.
6. Add the appliance wrapper and pichi orchestration against that interface.
7. Add attestation evidence without changing the standalone build semantics.

Validation should cover:

- Every supported recipe combination and prohibited `from` combination.
- Tool-carapace mutations never appearing as inherited output scutes.
- Firmware-only output and undefined `CARAPACE` behavior.
- Pairwise firmware replacement and propagation of other base components.
- Typed DTB encoding, wrong types, missing targets, and ordered operations.
- Regex zero/one/many matches, whitespace captures, and lazy variable expansion.
- Absent/authored/empty transforms, including missing default command lines.
- Locked digest selection despite moved cache tags and mismatched content.
- Git context isolation from subsequent host working-tree changes.
- Failure/cancellation cleanup and concurrent builds without device collisions.
- Hostile command attempts to signal, trace, or acquire descriptors for builder
  processes, including after mounting a fresh procfs.
- Outside-root access through inherited descriptors, symlinks, retained working
  directories, namespace handles, and deliberately malformed output paths.
- Builder IPC isolation, including abstract Unix sockets that have no filesystem
  path, and isolation between independently executing steps.
- Descendants that double-fork or create a new session, daemon hangs, and
  cancellation during filesystem requests.
- Stable output bytes after all untrusted writers are gone and worker writes
  have finished; no inventory acceptance on failed finalization.
- Real metadata behavior under enforcing SELinux where supported.
- OCI import round trips and inventory-to-output digest verification.
- Appliance transport tests separately from standalone privileged tests.

No VM is required to validate the main build pipeline. Namespace and device
tests may still require privileged Linux runners and their kernel facilities.

## 13. Remaining decisions

The initial implementation uses gix and verifies each consumed Git object's
identity before materialization. Recipe symlinks resolve only to captured files
within the context; hooks and checkout filters do not execute. The lock and
inventory wire formats, YAML values, device exposure, and supported firmware
measurement contracts are specified above. Temporary writable mappings use
kernel deferred removal and scratch leases for recovery after interrupted
activation. Ordinary tmpfs allocation failures propagate as build failures.

Remaining ecosystem decisions are authentication of the intended Git identity,
creation/override of components beyond carapace and firmware, additional
firmware signing/resealing contracts, and appliance transport and attestation.

The command-isolation redesign also requires selecting and validating the
filesystem backend, capability/syscall policy, outbound networking and DNS,
and independent failure-abort mechanism. Fully unprivileged storage assembly
remains under investigation. The trust model itself is agreed.

The initial scope excludes remote Git retrieval, uncommitted working-tree
inputs, submodules, Git LFS, cross-architecture builds, build-time lock updates,
output-cache mutation, firmware discard, partial PMI/DTB replacement,
transform-default inclusion, regex match-count options, whole-node operations,
and execution of `platform.yaml`. Caller working-tree policy is outside this
interface.
