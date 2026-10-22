# Safe Rust disabled-policy loader validation

The probe calls the production LabelPolicy<Prepared> -> LoadedDisabled API.
It loads no external object: build.rs compiles and embeds policy/labels.bpf.c.
The library opens the embedded object, verifies map layouts and program sections,
loads it through libbpf-rs, checks zero config and populates targets. The current
probe then exercises disabled hook attachment/pinning and cleanup. Config stays
zero throughout; this cannot enable the exception.

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
restoration and successful reload/drop. Both runs pass. v2 uses the then-current
formatted probe source and includes current source hashes; v1 preserves the
initial binary/input evidence without claiming final source identity.

v3 adds disabled pinning and observes one extra FD after cleanup. The witness
held a Vec<DirEntry>, which itself retains a read_dir descriptor on Linux. This
failed run and its probe source are preserved. v4 drops that witness before
counting; production code is unchanged between those runs. v4 passes explicit
close and Drop with all five expected pins present initially and an empty parent
afterward. It injects a directory collision at check_remove: config, targets and
check_set pins exist when installation fails. Cleanup also fails on that
collision, retains recovery ownership, then succeeds after the fixture removes
the collision. Descriptor counts return to baseline.

Descriptor accounting checks process-held resources, not a global kernel leak
proof. These tests do not enable policy or qualify durable anchors,
command containment, supervisor/guardian death,
concurrent builds, Arm or confidential hardware. Existing C-loader policy
semantics evidence remains in ../rewrite-label-policy; that is separate from
this production Rust loading test.
