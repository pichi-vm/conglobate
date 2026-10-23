# Conglobate

Conglobate builds layered OS images from committed recipes and locally supplied,
locked image artifacts. It is a standalone binary. The recipe pipeline supports
bootstrap, carapace layering, firmware replacement and DTB transformation, and
produces a self-contained OCI image layout with a CBOR input/output inventory.
See [DESIGN.md](DESIGN.md) for the CLI, schemas and inheritance rules.

The rewrite is undergoing qualification. [M7-QUALIFICATION.md](M7-QUALIFICATION.md)
records outstanding review, distro boot, architecture, confidential-platform and
CI coverage. A successful compilation or rootless test run does not establish
sandbox qualification.

## Build and rootless checks

Build on Linux with Rust/Cargo and a C build toolchain for native dependencies
such as zstd. The binary does not compile or load BPF, depend on libbpf/libelf,
or require Clang's BPF target or special Linux-header include paths. The trusted
execution environment supplies the fixed labeling exception described in
[SANDBOX-DESIGN.txt](SANDBOX-DESIGN.txt). Disposable-VM fixture tools build their
own environment policy separately and have additional compiler prerequisites.

The registration-free migration has 195 rootless tests and 50 doctests passing
with rustc/Cargo 1.94.0; retired policy-API tests remain preserved as historical
evidence. This does not qualify the composed sandbox. Earlier checks used rustc
1.99.0-nightly (1a98b1e13, 2026-08-07) and Cargo 1.98.0. These are tested
environments, not a minimum supported Rust version; that floor has not yet been
established.

After dependencies are available, run these commands as an ordinary user:

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo test --locked --doc
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
for dependency policy and known advisories. The first hosted run exposed the Ubuntu UAPI include-path requirement above;
the corrected rootless and dependency jobs pass at commit 148ef66
(GitHub run 39964805125). Privileged VM CI remains outstanding.
Local license/source checks pass for amd64 and arm64 dependency graphs; this is
not arm64 execution qualification. Advisory matching depends on package source
identities: a clean audit does not certify pinned Git revisions against ordinary
crates.io advisories. Exact results are recorded in the M7 review evidence.

## Runtime trust boundary

Run image builds only in a dedicated disposable VM with a qualified kernel and
an independent environment controller that destroys the VM if conglobate
crashes. The supervisor runs as initial-namespace root. Build commands are
untrusted and run in restricted namespaces; they receive neither supervisor
control resources nor backing block devices. The kernel, environment-provided BPF LSM labeling exception,
supervisor and supplied formatter runtime are trusted. The design requires
native filesystem/idmapped mount support and the environment-loaded labeling exception;
generic kernel version numbers alone do not establish those prerequisites.
The current implementation also uses `open_tree(OPEN_TREE_CLONE)` on subtrees of
its detached workspace. Ubuntu's GA 6.8 kernel rejects this operation. Hosted
lifecycle qualification currently uses HWE; an attached, protected workspace is
being evaluated to remove this implementation-specific dependency. This is not
an architectural requirement for command isolation.

[SANDBOX-DESIGN.txt](SANDBOX-DESIGN.txt) specifies the boundary and teardown
requirements. [ARCHITECTURE.md](ARCHITECTURE.md) explains Rust ownership and
typestate. Privileged integration tests must run inside disposable VMs, never
directly on the development host. The retained integration harness builds a
fresh guest from trusted local tools and explicit kernel/module/ORAS inputs;
provisioning those inputs in CI remains an outstanding M7 task.

Conglobate does not download missing Git objects or image artifacts. Recipe
commands have networking disabled by default; enabled commands require an
externally prepared, build-dedicated network namespace (`--network-namespace PATH`)
and an explicit resolver file (`--resolv-conf PATH`). The environment owns network
interfaces, routes, outbound NAT, firewall rules, DNS services and namespace
lifetime. Conglobate pins the namespace once, joins it for enabled commands and
temporarily mounts the captured resolver configuration read-only. Disabled
commands receive a fresh disconnected namespace. Conglobate does not run a DNS
relay or configure the external network, and requires no particular service manager.
Artifact preparation, appliance construction and attestation also belong to the
surrounding environment.

## Implementation and evidence

Application Rust forbids unsafe code. Prefer methods on meaningful owning types,
RAII, sealed marker-trait states, immutable validated inputs and consuming
transitions. Cleanup failures must preserve resource ownership rather than claim
successful finalization. See [REIMPLEMENTATION.md](REIMPLEMENTATION.md) for the
milestones and required adherence gates.

The old implementation and exploratory validation remain preserved as reference
inputs until replacement evidence and recoverability have been verified. They
are not competing specifications or completed qualification for the rewrite.
