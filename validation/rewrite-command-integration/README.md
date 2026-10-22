# Integrated command execution evidence

This fixture combines production private mount/idmapped attachment, registered
cgroup containment, helper observation, cancellation and child reaping with an
actual shell running in an ext4 image. It is not the production executor or full
sandbox qualification. All mounts, namespace and capability operations occur
inside a disposable QEMU guest; the host only builds and packages the fixture.
The guest has no host disk/share or network device.

## Reproduction

Requires the preserved seed at
`/var/tmp/conglobate-cap-review-fix-normal-v1`, Fedora kernel modules for
`7.2.10-200.fc44.x86_64`, QEMU and the tools named in run.py. Build with:

```
TMPDIR=/var/tmp/conglobate-rewrite-tmp cargo build --locked --manifest-path validation/rewrite-command-integration/Cargo.toml
python3 validation/rewrite-command-integration/run.py /var/tmp/NEW-UNUSED-DIRECTORY
```

The seed supplies the guest initramfs tools and QEMU arguments. Per-run input
hashes, serial logs, command, source snapshots and verdicts are in evidence/.
Large guest artifacts remain at the paths in provenance.json; these temporary
paths must be durably archived before reference retirement. This harness has its
own pinned Cargo.lock. Sources use safe Rust and methods on the fixture object.

## Results

- v1: initial successful command write and timeout/cgroup/reaping checks.
- v2: stronger privilege assertions failed before command startup; no full pass.
- v3: traced failure: Bubblewrap --cap-drop ALL left a nonzero bounding set.
- v4: explicit setpriv finalization cleared the bounding set before the shell.
  Both command cases, filesystem check and remount passed; fixture cleanup failed
  because its initramfs lacked rmdir. The overall run correctly failed.
- v5: added rmdir; all assertions and fixture cleanup passed. Clippy with warnings
  denied and formatting also pass for the fixture.

v5 checks all five capability sets are zero, no_new_privs is one, UID mapping is
0 -> 100000 for 65536 IDs, selected supervisor paths are absent, and shell writes
appear as UID 0 in the original filesystem. It checks normal completion and a
200ms timeout, domain removal, helper reaping, no remaining adopted children,
unchanged supervisor sentinel, clean unmount, e2fsck and read-only remount data.
These are benign assertions over owned fixture resources, not escape attempts.

## Limits and implications for production

Bubblewrap, nsenter and setpriv are temporary test adapters. The fixture adds
SETPCAP solely to its trusted finalizer and drops the bounding set before exec;
the command shell has no capabilities. This is not the final OS-build allowlist.
The finalizer is copied into this fixture's freshly constructed image, whose
contents are known. Production must not execute a trusted finalizer from an
untrusted image path or rely on its loader/libraries; it needs trusted executable
and dependency authority until command admission.

No scoped labeling authorization, final seccomp policy, metadata/SELinux package
transaction, network-enabled step, dual-mount bootstrap or direct scute sealing
is integrated here. Tests do not establish complete FD/path confinement. The
fixture discovers a generated mount path; production requires a scoped handle.
The timeout kills the helper and leaves an empty mount-point directory, which
fixture cleanup removes only after unmount. Production ordinary-error ownership
must handle this without a serialized next-invocation recovery requirement.
RecordedDomain is reused as an existing containment adapter, not an endorsement
of the superseded guardian architecture. The v6 supervisor-death case below demonstrates VM disposal with live commands.
Failed startup aborts
the guest fixture, so partial-startup cleanup still needs integrated coverage.


## Dedicated-VM supervisor death

Run with a new output directory and `--supervisor-death` as the final argument.
v6 starts a continuously running contained command, then its independent guest
controller terminates the supervisor with SIGKILL. It verifies wait status 137
and a still-populated command cgroup, then returns to guest PID 1, which powers
off the VM through sysrq. QEMU exits; the serial log includes kernel Power down.
There is no guardian or next invocation, and no command-success marker appears.
The controller and PID 1 are outside the supervisor's process/cgroup lifetime.

This demonstrates the fixture environment contract, not the eventual appliance's
integration. No inventory/publication implementation is exercised; absence of a
success marker is not artifact rejection coverage. Active-policy lifetime during
the shutdown interval remains outstanding. The helper may survive until VM
shutdown by design. v7 reruns normal completion, timeout and filesystem checks
with the same updated sources and passes. Both runs and their init scripts are
preserved; Clippy and formatting pass.
