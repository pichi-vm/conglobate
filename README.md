# Conglobate

Conglobate builds layered OS images from committed plans and locally supplied,
locked image artifacts. The crate is a types-only library plus a binary engine;
the standalone CLI remains testable without a VM. Production builds run in a
pichi appliance image shipped from this repository (see DESIGN.md). The
plan format is the shared staged model in [DESIGN.md](DESIGN.md): named
creation and extension stages, followed by selection of the root and boot
components. The engine executes only the selected image's dependency closure.
Builds produce a self-contained OCI image layout with build binding annotations
on the image manifest.

## Plans

A plan requires `version: 1` and an `image` selection. The optional `stages`
map names the creation and extension stages:

```yaml
version: 1
stages:
  package:
    use: registry.example.com/tools:1
    steps:
      - exec: {command: "printf 'built input\n' > /mnt/package"}
  rootfs:
    extend: registry.example.com/base:1
    steps:
      - copy: {from: package, path: /package, into: /etc/package}
image: rootfs
```

The separate lock maps each remote reference directly to an OCI index digest.
Local references name stages and never appear in the lock. Dependencies execute
before their consumers, regardless of YAML key order. Unselected stages do not
execute or acquire cache inputs.

Creation (`use`) runs tools at `/` and populates a newly formatted root at
`/mnt`; tool writes are discarded. Extension (`extend`) modifies its input
root at `/` and retains its boot components. Each stage adds one scute.
Optional `format` controls ext4 capacity, label and ESP size. Explicit tmpfs
and ESP mounts persist across that stage's directives.

COPY without `from` reads the committed context. Named COPY reads a completed
stage's persisted root; it does not replay temporary stage mounts. See
[directives](DESIGN.md#63-directives) for path and copy rules.

Generate the image lock with `pichi update` from the working-tree plan, then
commit the plan and lock together before building. Pichi consumes the shared
plan types directly; lock maintenance does not invoke the Conglobate CLI.

`image: rootfs` forwards that image's components. Explicit composition selects
root and boot independently; boot can be inherited or extracted from a PMI
file in a source image. Native bootconfig accompanies the immutable PMI and
supports literal definitions and overlays. An existing `corium.root` binds
configuration to the selected root. See [image assembly](DESIGN.md#52-image-assembly)
and [boot configuration](DESIGN.md#8-boot-configuration).

An optional `requirements` section replaces the output's launch contract:

```yaml
requirements:
  slot: pci
  cpus: {required: 2, recommended: max}
  memory: {required: 2048, recommended: max}
  storage: {required: 10240, recommended: max, slot: pci}
  interfaces:
    public:
      description: Application traffic
      slot: mmio
      ingress:
        required: [443]
        recommended: [80, [8000, 8100], all]
```

Memory and storage are in MiB. Storage describes the single mutable overlay
above the read-only image. `max` requests the largest allocation the operator
permits; `all` is a wildcard entry within a port list. A resource's slot type
overrides the image's global default. Declared transports are mandatory: the
host must realize them or fail. These fields are recorded in the output's
version-1 config blob. Pichi resolves policy into explicit per-device buses;
Dillo implements that specification without transport substitution. See [launch requirements](DESIGN.md#52-image-assembly) for semantics.

The 28 October 2026 decision in
[DESIGN §11](DESIGN.md#11-artifact-and-build-annotations) drops `inventory.cbor`
entirely. The image manifest carries `dev.pichi.build.commit` (`sha1:<hex>` or
`sha256:<hex>`), `dev.pichi.build.context` (path in that commit), and
`dev.pichi.build.plan` / `dev.pichi.build.lock` (paths relative to context).
`dev.pichi.build.location` is an optional provenance hint. There are no separate
tree/object-format keys or builder/input/execution/output mirrors. The carapace
tip root is recorded once as `sha256:<hex>` in `dev.pichi.carapace.verity.hash`.
Per-scute
salt/data-blocks annotations let Pichi reconstruct the verity trees. Each newly
sealed scute uses its parent root followed by a fresh 32-byte cryptographically
random suffix as its salt. Reused layers keep their existing salts.
[Conglobate #24](https://github.com/pichi-vm/conglobate/issues/24) removes
`build.yaml` and flat packing-list copies after host OCI import landed in
[Pichi #28](https://github.com/pichi-vm/pichi/pull/28) (`7df44442`). Builds
leave only `oci-layout`, `index.json`, and `blobs/sha256/…`. Later attestation
binds the image digest and TEE/builder.

Build publication stamps these annotations from the resolved committed input.
The library owns the portable plan, lock and artifact types. The engine
validates execution constraints and dependency ordering, then publishes the
complete OCI layout. Its dependency module is `src/graph.rs`.

## Build and rootless checks

Build on Linux with Rust/Cargo and a C build toolchain for native dependencies
such as zstd. The binary does not compile or load BPF, depend on libbpf/libelf,
or require Clang's BPF target or special Linux-header include paths. The trusted
execution environment supplies the fixed labeling exception described in
[sandbox design](DESIGN.md#14-sandbox-lifecycle).

CI builds and tests with Rust 1.94.0 on native amd64 and Arm64. This is the tested
toolchain, not a separately established minimum supported Rust version.

The library has no default features. Enable `cli` to build the executable.

After dependencies are available, run these commands as an ordinary user:

```sh
cargo build --locked --features cli --bin conglobate
cargo fmt --all -- --check
cargo clippy --locked --features cli --all-targets -- -D warnings
cargo test --locked --features cli --all-targets
cargo check --locked --no-default-features --lib
```

Use `--offline` as well when all locked dependencies are already cached. The
rootless tests require a writable `/dev/shm` tmpfs for RAM-backed materialization;
ordinary disk-backed temporary storage is not a substitute. Set `TMPDIR` to an
existing writable directory if the compiler's default temporary location is
unsuitable. Builds may use disk-backed compiler temporary storage.

Conglobate creates `/var/tmp/conglobate` and uses private per-build directories
there for anonymous image scratch and snapshot backing. It uses that location
without inspecting its backing filesystem; the environment supplies storage.
Scratch files are removed on last close, and snapshot capacity is bounded by
available backing space. Small runtime data and helpers use the private control
tmpfs exposed under `/run/conglobate`; the service provisions that parent with
`RuntimeDirectory=conglobate`. Output staging stays on the output filesystem.

Serialize Cargo checks and use `CARGO_BUILD_JOBS=2` in constrained development
environments. The rootless workflow uses Rust 1.94.0 and runs formatting, Clippy,
tests and doctests. A separate job runs pinned cargo-deny and cargo-audit tools
for dependency policy and known advisories. Advisory
matching depends on package source identities; clean reports do not certify
pinned Git revisions against every crates.io advisory.

## Runtime trust boundary

Production image builds run in the dedicated disposable **build appliance** (a
pichi image shipped from this repo) with a qualified kernel and an independent
environment controller that destroys the VM if conglobate crashes. systemd is
PID 1; static virtiofs mounts expose `repo`/`cache`/`output` at
`/run/pichi/{repo,cache,output}`; the guest opens named console
`dev.pichi.build`, writes `["build", 1, null]`, then reads one
self-delimiting host CBOR response. The supervisor runs as initial-namespace root. Build
commands are untrusted and run in restricted namespaces; they receive neither
supervisor control resources nor backing block devices. The kernel,
environment-provided BPF LSM labeling exception (pure-Rust Aya in the trusted
environment; see the sandbox design), supervisor and **image-resident
formatters** (internal filesystem-name → formatter map; formatting outside the
command sandbox) are trusted. There is no host `--formatter-runtime` TEE input.
The design requires native filesystem/idmapped mount support; generic kernel
version numbers alone do not establish those prerequisites. The current
implementation also uses `open_tree(OPEN_TREE_CLONE)` on subtrees of its detached
workspace. Ubuntu's GA 6.8 kernel rejects this operation.

The **standalone CLI** remains first-class for rootless checks and engine tests
without a VM. [sandbox design](DESIGN.md#14-sandbox-lifecycle) specifies the
boundary and teardown requirements. [Rust architecture](DESIGN.md#15-rust-architecture)
explains the types-only lib, binary engine, ownership and typestate.

Conglobate does not download missing Git objects or image artifacts.
Artifact preparation and attestation belong to the surrounding environment;
the appliance image itself is built from this repository.

## Implementation style

Application Rust forbids unsafe code. Prefer methods on meaningful owning types,
RAII, sealed marker-trait states, immutable validated inputs and consuming
transitions. Cleanup failures must preserve resource ownership rather than claim
successful finalization. [Rust architecture](DESIGN.md#15-rust-architecture)
describes the types-only library, binary engine, module boundaries and ownership
model. Maintained regression fixtures live under `tests/`.

## Contributing

### Commit messages

Commits follow Conventional Commits and standard git formatting. The gitlint
configuration is in `.gitlint`; CI checks the current commit message. The
rootless workflow also checks Rust formatting, Clippy, tests and doctests on
amd64 and Arm64.

- Subject: `type(scope): summary` — lowercase, imperative, no trailing period,
  at most 50 characters. Types: `feat`, `fix`, `docs`, `style`, `refactor`,
  `perf`, `test`, `build`, `ci`, `chore`, `revert`, `spec`.
- One blank line, then a body wrapped at 72 characters explaining what changed
  and why (not how — the diff shows how).
- Write each message to stand on its own: describe the change and its rationale
  for a reader with no knowledge of the project's history or how the work was
  produced. Do not reference internal process, prior iterations, or review
  back-and-forth. (gitlint cannot check this — it is a review responsibility.)
