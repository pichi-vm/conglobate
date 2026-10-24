# Conglobate

Conglobate builds layered OS images from committed recipes and locally supplied,
locked image artifacts. It is a standalone binary. The recipe pipeline supports
bootstrap, carapace layering, firmware replacement and DTB transformation, and
produces a self-contained OCI image layout with a CBOR input/output inventory.
See [DESIGN.md](DESIGN.md) for the CLI, schemas and inheritance rules.

The standalone rewrite is implemented and qualified in the environments recorded
in [QUALIFICATION.md](QUALIFICATION.md). Fedora, Debian, Alpine and NixOS example
recipes live under `tests/distro`. Confidential-computing qualification and
appliance attestation are deferred; passing tests are not a universal security
or operating-system compatibility claim.

## Build and rootless checks

Build on Linux with Rust/Cargo and a C build toolchain for native dependencies
such as zstd. The binary does not compile or load BPF, depend on libbpf/libelf,
or require Clang's BPF target or special Linux-header include paths. The trusted
execution environment supplies the fixed labeling exception described in
[SANDBOX-DESIGN.txt](SANDBOX-DESIGN.txt). Disposable-VM fixture tools build their
own environment policy separately and have additional compiler prerequisites.

CI builds and tests with Rust 1.94.0 on native amd64 and Arm64. This is the tested
toolchain, not a separately established minimum supported Rust version.

After dependencies are available, run these commands as an ordinary user:

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
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
lifecycle tests use Ubuntu HWE kernels; the recorded kernel and feature checks
are the supported evidence, rather than a generic minimum-version promise.

[SANDBOX-DESIGN.txt](SANDBOX-DESIGN.txt) specifies the boundary and teardown
requirements. [ARCHITECTURE.md](ARCHITECTURE.md) explains Rust ownership and
typestate. Privileged integration tests must run inside disposable VMs, never
directly on the development host. The maintained harness builds a
fresh guest from trusted local tools and explicit kernel/module inputs. CI
provisions these independently of the conglobate binary.

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

The old implementation and exploratory validation are preserved outside the
active tree in verified archives; see [ARCHIVE.md](ARCHIVE.md) for recovery.
Selected acceptance records remain in `evidence/`.
