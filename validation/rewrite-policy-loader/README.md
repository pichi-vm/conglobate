# Safe Rust disabled-policy loader validation

The probe calls the production LabelPolicy<Prepared> -> LoadedDisabled API.
It loads no external object: build.rs compiles and embeds policy/labels.bpf.c.
The library opens the embedded object, verifies map layouts and program sections,
loads it through libbpf-rs, checks zero config and populates targets. No hook is
attached and no map/link pin is created; this cannot enable the exception.

Build and run (new output directory required):

```
TMPDIR=/var/tmp/conglobate-rewrite-tmp cargo build --locked --manifest-path validation/rewrite-policy-loader/Cargo.toml
TMPDIR=/var/tmp/conglobate-rewrite-tmp python3 validation/rewrite-policy-loader/run.py /var/tmp/NEW-loader
```

The runner derives from the preserved mount-api VM fixture. It uses the same
local seed, kernel and tooling prerequisites, copies the compiled probe and its
shared libraries, then runs it in a no-NIC/no-disk/no-share QEMU guest. Kernel
operations, capability changes and temporary mounts occur only in that guest.
The marker check prevents accidental invocation outside the fixture; it is not
an authorization mechanism.

Evidence v1 covers three production load/drop cycles with the process descriptor
count returning to baseline. Evidence v2 adds clearing only the probe thread's
effective capabilities, a failed load with no descriptor leak, capability
restoration and successful reload/drop. Both runs pass. v2 uses the final
formatted probe source and includes current source hashes; v1 preserves the
initial binary/input evidence without claiming final source identity.

Descriptor accounting checks process-held resources, not a global kernel leak
proof. These tests do not attach/enable policy or qualify partial attachment,
pin failure, durable anchors, command containment, supervisor/guardian death,
concurrent builds, Arm or confidential hardware. Existing C-loader policy
semantics evidence remains in ../rewrite-label-policy; that is separate from
this production Rust loading test.
