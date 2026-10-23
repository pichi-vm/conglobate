# Maintained privileged lifecycle fixtures

This separate safe-Rust binary exercises production library owners in disposable
VMs. It is not a runtime dependency. Do not run privileged cases on the host.
The entrypoint refuses to proceed without the guest marker, including on trusted
helper re-exec. The marker is an accidental-use guard, not a security boundary.

Initial migration: `formatter-failures`, copied without assertion changes from
`validation/rewrite-command-integration/src/formatter_failures.rs` at 5ee7018.
It tests formatter deadline cancellation/reaping, exhausted writable-snapshot
rejection and explicit resource release. `FORMATTER_DROP=running` and `wait`
are terminal VM-disposal cases that check retained storage on abandonment.
The stalled formatter parks; synchronization does not use sleeps.

Compile with ordinary user privileges:

```
CARGO_BUILD_JOBS=2 TMPDIR=/var/tmp/conglobate-rewrite-tmp \
  cargo +stable check --manifest-path tests/lifecycle/Cargo.toml --locked
```

Compilation is not validation. Run the maintained disposable-VM runner with a
new output directory and a qualified local kernel/module tree:

```sh
python3 tests/lifecycle/run.py /var/tmp/NEW-LIFECYCLE-RUN \
  --kernel /path/to/vmlinuz --kernel-release RELEASE --module-root /
```

Use separate new directories with `--drop running` or `--drop wait` for the
terminal retention cases. The runner stages trusted host formatter tools and
the fixture, builds a disconnected native amd64/KVM guest, and preserves source,
input hashes, kernel identity, serial, raw status and assessment. Success requires
case assertions, status-disk exit zero and VM poweroff. It does not run BPF or
mount/device operations on the host. These trusted-formatter tests do not exercise
command labeling, so no LSM exception is needed for this subset.

The runner currently reuses the historical rootless GuestRuntime constructor;
that dependency must be migrated before M8 retirement. The original runner and
all historical results remain unchanged. Large run artifacts must be durably
archived before deletion; a temporary run path is not sufficient preservation.

The original harness and evidence remain intact. It currently fails to compile
against removed policy/pin APIs. Remaining command/network/bootstrap/publication
cases need explicit migration; none is considered retired or covered merely
because this subset compiles. Networking tests must use retained namespace
ownership without recreating policy-specific pins. Future tests must preserve
actual production ownership boundaries, not widen descriptor APIs for convenience.

`--case domain-publication` migrates the original domain-record rollback probe:
absent, published, malformed and identity-mismatched records. It uses real empty
cgroup-v2 domains and injects a publication error through the public failure
owner. Malformed/mismatched cases must reject cleanup while retaining domain and
record; restoring the fixture's own correct record then permits rollback. Every
case also checks no child cgroup remains. No command runs and no LSM registration
is involved. These are in-process partial-setup tests, not recovery across builds.
