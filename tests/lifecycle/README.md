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

Compilation is not validation. A maintained disposable-VM runner and fresh runs
are still required before these migrated cases qualify the current implementation.
The source expects a trusted formatter runtime manifest, a copy of this binary
as `/usr/bin/formatter-stall`, guest-only device-mapper modules and the marker.
The historical runner documents the packaging precedent; it is not compatible
with this new binary without adaptation.

The original harness and evidence remain intact. It currently fails to compile
against removed policy/pin APIs. Remaining command/network/bootstrap/publication
cases need explicit migration; none is considered retired or covered merely
because this subset compiles. Networking tests must use retained namespace
ownership without recreating policy-specific pins. Future tests must preserve
actual production ownership boundaries, not widen descriptor APIs for convenience.
