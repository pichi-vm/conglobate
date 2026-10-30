# Conglobate

Conglobate builds layered OS images from committed plans and locally supplied,
locked image artifacts. The crate is a types-only library plus a binary engine;
the standalone CLI remains testable without a VM. Production builds run in a
pichi appliance image shipped from this repository (see DESIGN.md). The
approved plan format is the multi-stage task model in
[DESIGN.md](DESIGN.md): tasks `bootstrap`, `extend`, and `build`, then
assembly of one carapace and optional boot. The engine executes only the
selected image's dependency closure. Builds produce
a self-contained OCI image layout with build binding annotations on the image
manifest. See [DESIGN.md](DESIGN.md) for the task model, CLI, appliance
build-port CBOR contract, and schemas.

Fedora, Debian, Alpine and NixOS example plans live under `tests/distro`. Confidential-computing qualification and
appliance attestation are deferred; passing tests are not a universal security
or operating-system compatibility claim.

## Plans

A plan has named `tasks` and an explicit local `image` selection:

```yaml
tasks:
  package:
    kind: build
    from: registry.example.com/tools:1
    exec:
      - run: printf 'built input\n' > /package
    into: {package: /package}
  rootfs:
    kind: extend
    from: registry.example.com/base:1
    exec:
      - copy: {from: package, into: /etc/package}
image: rootfs
```

The separate lock pins each remote reference to an OCI index digest. Local
references name tasks and never appear in the lock. Orphan tasks require no
cache inputs, networking, or execution. Dependencies run before their consumers,
regardless of YAML key order. `bootstrap` formats a fresh filesystem and populates
`/mnt`; `extend` retains its input chain plus one scute for all its commands;
`build` discards its tool writes and retains declared artifacts only.

Before resolving a lock, inspect the committed plan with:

```sh
conglobate inputs --repository . --commit "$(git rev-parse HEAD)" \
  --context . --plan pichi.yaml
```

This prints a JSON object with an `images` array of unique remote references
from the selected task closure. It uses the build's context reader and planner;
no lock, cache, privileged setup, or commands are needed. Pichi consumes this
output and resolves each reference to an OCI index pin.

`image: rootfs` uses that task's carapace and boot exports. The long form
`image: {root: rootfs, boot: firmware}` selects them separately. A build task
may provide boot without a carapace. Extend carries boot forward and automatically
rewrites its DTB binding; assembly also binds separately imported boot to the
selected root. There is no authored `transform` section.
`CARAPACE` becomes available to commands after the selected image-root task
completes. Commands that produce that root see it unset; input, ancestor and
tool roots never substitute for the output identity. Boot-only builds leave it
unset even when a local task supplies their tools.

Some artifact wire details remain provisional implementation choices under
[DESIGN §6.3](DESIGN.md#63-exec-from-into-and-copy), not additional design locks:
`tasks` contains the map; `into` maps flat names to absolute regular-file paths;
`copy.into` is an absolute destination. An explicit `pmi`/`dtb` pair in `into`
provides boot. Directory exports and other value grammars fail closed. Context
COPY selects a single bare file or directory name; source patterns and the old
owner/group/mode fields are rejected. Artifact names conflicting with the context
or another producer are rejected. Whether artifact keys should become paths is
still open. These choices are isolated in the executable's private plan schema.

The 28 October 2026 decision in
[DESIGN §11](DESIGN.md#11-artifact-and-build-annotations) drops `inventory.cbor`
entirely. The image manifest carries `dev.pichi.build.commit` (`sha1:<hex>` or
`sha256:<hex>`), `dev.pichi.build.context` (path in that commit), and
`dev.pichi.build.plan` / `dev.pichi.build.lock` (paths relative to context).
`dev.pichi.build.location` is an optional provenance hint. There are no separate
tree/object-format keys or builder/input/execution/output mirrors. The carapace
tip root is derived from the manifest; never stamp or require
`dev.pichi.carapace.verity.hash`. Existing verity parameters and per-scute
salt/data-blocks annotations stay.
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
`dev.pichi.build`, writes `{"v":1,"op":"build-input"}`, then reads one
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
