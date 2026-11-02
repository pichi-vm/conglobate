# Conglobate

Conglobate builds layered OS images from committed plans and locally supplied,
locked image artifacts. The crate is a types-only library plus a binary engine;
the standalone CLI remains testable without a VM. Production builds run in a
pichi appliance image shipped from this repository (see DESIGN.md). The
plan format is defined in [DESIGN.md](DESIGN.md). Named stages create or extend
image roots; the top-level image declaration selects the root, PMI and optional
bootconfig. The engine executes only the selected dependency closure and emits
a self-contained OCI layout with build annotations on its manifest.

Fedora, Debian, Alpine and NixOS example plans live under `tests/distro`. Confidential-computing qualification and
appliance attestation are deferred; passing tests are not a universal security
or operating-system compatibility claim.

## Staged recipe library

`conglobate::recipe` exposes portable serde types for staged images. Each type
lives in its own module. `Plan::validate()` checks every declaration and
reference; `Plan::order()` returns only the selected dependency closure.
Runtime dependencies remain behind `cli`. The executable decodes the same types.
See [the staged recipe example](tests/fixtures/staged-recipe.yaml).

Stages either create a root with `using`, `format` and nonempty `derive`, or
extend a root with `extending` and nonempty `derive`. Directives contain `exec`
or `copy`. COPY reads the unnamed host context unless `from` names a stage.
Mount targets belong to the output image namespace. The top-level `image`
forwards one source or selects `root`, `boot` and `bootconfig` explicitly.
Bootconfig requires a selected PMI; omitting its `from` starts empty.

## Plans

A plan has a `stages` map and an `image` selection:

```yaml
stages:
  package:
    extending: registry.example.com/tools:1
    derive:
      - exec: printf 'built input\n' > /package
  rootfs:
    extending: registry.example.com/base:1
    mount:
      /tmp: tmpfs
    derive:
      - copy: {from: package, path: /package, into: /etc/package}
      - exec: cat /etc/package
image: rootfs
```

The separate lock pins remote stage inputs and direct image selectors to OCI
index digests. Local references name stages. Every declaration is validated;
only the selected dependency closure fetches inputs and executes. Dependencies
run before consumers regardless of YAML map order. A creation stage uses
`using` and `format`, populates a new GPT root at `/mnt`, and emits that root
alone. An extension uses `extending`, changes its root at `/`, and retains its
PMI and bootconfig. Both require nonempty `derive` and produce one scute.

Before resolving a lock, inspect the committed plan with:

```sh
conglobate inputs --repository . --commit "$(git rev-parse HEAD)" \
  --context . --plan pichi.yaml
```

This prints the selected unique remote references as an `images` JSON array.
No lock, cache or privileged runtime is needed. Pichi resolves those references
and supplies the cache; it does not interpret YAML plans on the host.

`image: rootfs` forwards all available components from that stage. The same
shortcut accepts a qualified remote. Explicit composition selects each component:

```yaml
image:
  root: rootfs
  boot: {from: package, path: /boot/image.pmi}
  bootconfig:
    from: registry.example.com/base:1
    values:
      corium.root: ""
      corium.log: info
      corium.halt: null
      kernel.console: [ttyS0, hvc0]
      kernel.quiet: []
```

Here `boot` extracts a regular PMI file from a persisted image root. A scalar
`boot` instead selects an existing image's PMI. Omitted components are absent;
bootconfig requires PMI. Omitting bootconfig's `from` starts empty. Strings,
string lists, bare keys (`[]`), empty strings, and exact-key deletion (`null`)
remain distinct. An existing `corium.root` is rebound after an overlay when new
root layers or a different root are joined. Unchanged-root overlays retain the
authored binding. PMI bytes and their embedded command line are never rewritten.
Updating Corium code requires a new PMI.

COPY without `from` reads the captured Git context. With `from`, it reads a
named stage's whole persisted root; stage mounts are not replayed. Destination
paths belong to the execution root (`/mnt/...` during creation). ESP and tmpfs
mount targets always belong to the output image namespace. Mounts span the
stage's commands and copies; ESP writes persist, tmpfs writes disappear.

The 28 October 2026 decision in
[DESIGN §11](DESIGN.md#11-artifact-and-build-annotations) drops `inventory.cbor`
entirely. The image manifest carries `dev.pichi.build.commit` (`sha1:<hex>` or
`sha256:<hex>`), `dev.pichi.build.context` (path in that commit), and
`dev.pichi.build.plan` / `dev.pichi.build.lock` (paths relative to context).
`dev.pichi.build.location` is an optional provenance hint. There are no separate
tree/object-format keys or builder/input/execution/output mirrors. The carapace
tip root is recorded once in `dev.pichi.carapace.verity.hash`. Per-scute
salt/data-blocks annotations let Pichi reconstruct the verity trees. Each newly
sealed scute uses its parent root followed by a fresh 32-byte cryptographically
random suffix as its salt. Reused layers keep their existing salts.
[Conglobate #24](https://github.com/pichi-vm/conglobate/issues/24) removes
`build.yaml` and flat packing-list copies after host OCI import landed in
[Pichi #28](https://github.com/pichi-vm/pichi/pull/28) (`7df44442`). Builds
leave only `oci-layout`, `index.json`, and `blobs/sha256/…`. Later attestation
binds the image digest and TEE/builder.

Build publication stamps these annotations from the resolved committed input.
Plan parsing, filesystem properties, and dependency ordering are private to
the executable. The library does not expose legacy `pichi.build` plans or
packing lists. Builds publish OCI layouts; consumers use the shared artifact
types. The engine dependency module is `src/graph.rs`.

## Build and rootless checks

Build on Linux with Rust/Cargo and a C build toolchain for native dependencies
such as zstd. The binary does not compile or load BPF, depend on libbpf/libelf,
or require Clang's BPF target or special Linux-header include paths. The trusted
execution environment supplies the fixed labeling exception described in
[sandbox design](DESIGN.md#14-sandbox-lifecycle). That environment path is a
pure-Rust Aya program together with Rust load, attach and pin. Conglobate does
not perform those steps. The C fixtures `tests/environment/labels.bpf.c`,
`tests/environment/loader.c` and related C loaders such as
`tests/distro/labeling-fault/loader.c` are an interim harness for disposable-VM
qualification only. Fixture tools build that interim policy separately and
have additional compiler prerequisites until the Aya path replaces them.

CI builds and tests with Rust 1.94.0 on native amd64 and Arm64. This is the tested
toolchain, not a separately established minimum supported Rust version.

The library has no default features. Enable `cli` to build the executable;
`test-fixtures` enables it automatically for the internal VM fixtures.

After dependencies are available, run these commands as an ordinary user:

```sh
cargo build --locked --features cli --bin conglobate
cargo fmt --all -- --check
cargo clippy --locked --features cli --all-targets -- -D warnings
cargo test --locked --features cli --all-targets
python3 tests/check_contract.py
cargo check --locked --features test-fixtures --bin conglobate-lifecycle-tests
```

Use `--offline` as well when all locked dependencies are already cached. The
rootless tests require a writable `/dev/shm` tmpfs for RAM-backed materialization;
ordinary disk-backed temporary storage is not a substitute. Set `TMPDIR` to an
existing writable directory if the compiler's default temporary location is
unsuitable. Builds may use disk-backed compiler temporary storage; build
workspace storage inside conglobate is separately required to be RAM-backed.

Serialize Cargo checks and use `CARGO_BUILD_JOBS=2` in constrained development
environments. The rootless workflow uses Rust 1.94.0 and runs formatting, Clippy,
tests and doctests. A separate job runs pinned cargo-deny and cargo-audit tools
for dependency policy and known advisories. Disposable-VM lifecycle CI runs on
both architectures; Arm64 uses explicit TCG emulation when hosted KVM is absent.
That is a functional test fixture, not a proposed production sandbox. Advisory
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
The design requires native filesystem/idmapped mount support and the
environment-loaded labeling exception; generic kernel version numbers alone do
not establish those prerequisites. The current implementation also uses
`open_tree(OPEN_TREE_CLONE)` on subtrees of its detached workspace. Ubuntu's GA
6.8 kernel rejects this operation. Hosted lifecycle tests use Ubuntu HWE
kernels; the recorded kernel and feature checks are the supported evidence,
rather than a generic minimum-version promise.

The **standalone CLI** remains first-class for rootless checks and engine tests
without a VM. [sandbox design](DESIGN.md#14-sandbox-lifecycle) specifies the
boundary and teardown requirements. [Rust architecture](DESIGN.md#15-rust-architecture)
explains the types-only lib, binary engine, ownership and typestate. Privileged
integration tests must run inside disposable VMs, never directly on the
development host. The maintained harness builds a fresh guest from trusted local
tools and explicit kernel/module inputs. CI provisions these independently of
the conglobate binary.

Conglobate does not download missing Git objects or image artifacts. Plan
commands have networking disabled by default. On the standalone CLI, enabled
commands require an externally prepared, build-dedicated network namespace
(`--network-namespace PATH`) and an explicit resolver file (`--resolv-conf PATH`).
In the appliance image those paths are hardcodes, not CBOR fields. The
environment owns network interfaces, routes, outbound NAT, firewall rules, DNS
services and namespace lifetime. Conglobate pins the namespace once, joins it for
enabled commands and temporarily mounts the captured resolver configuration
read-only. Disabled commands receive a fresh disconnected namespace. Conglobate
does not run a DNS relay or configure the external network.
Artifact preparation and attestation belong to the surrounding environment;
the appliance image itself is built from this repository.

## Implementation style

Application Rust forbids unsafe code. Prefer methods on meaningful owning types,
RAII, sealed marker-trait states, immutable validated inputs and consuming
transitions. Cleanup failures must preserve resource ownership rather than claim
successful finalization. [Rust architecture](DESIGN.md#15-rust-architecture)
describes the types-only library, binary engine, module boundaries and ownership
model. Maintained regression fixtures live under `tests/`.

See [tests/README.md](tests/README.md) for disposable-VM harness setup and
fixture-specific instructions.

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

The portable `artifact::BootConfig` type parses native Linux configuration and
writes deterministic text. It retains bare keys, empty values and arrays; it
accepts nested keys, comments, `:=` replacement and `+=` append syntax on input.
`recipe::BootConfig::apply` applies YAML values to a resolved base: null deletes
only that key, and arrays replace the complete previous value. Linux has no
quote escapes; values that cannot be represented faithfully are errors.

Images carry native text as `application/vnd.pichi.bootconfig.v1`, alongside
exactly one PMI. Empty images and standalone boot configurations are invalid.
An empty merged configuration has no descriptor. Input validation authenticates
config bytes, checks Linux limits, and checks every PMI target's reservation,
including native padding and footer space. The engine validates PMI architecture and reservations before publishing.
Bootconfig is native text; it is not a detached DTB.
