# Rewrite label-policy validation

These tests load the actual `policy/labels.bpf.c` asset in disposable x86-64
QEMU/KVM guests. They do not load BPF, mount filesystems or install packages on
the host. Guests have no NIC, host disk or shared directory. They reuse the
preserved kernel-mount fixture and its offline Fedora package workload.

The test-only C loader populates one or two target entries, pins both maps and
all three links with config initially disabled, then enables the subject. The
loader is killed before commands run; the fixture retains namespace and mount
lifetimes and disables/removes policy after command quiescence. This remains
fixture lifecycle evidence, not qualification of a production Rust guardian.

Run with a new output directory:

```
TMPDIR=/var/tmp/conglobate-rewrite-tmp python3 validation/rewrite-label-policy/run.py /var/tmp/NEW-single
TMPDIR=/var/tmp/conglobate-rewrite-tmp python3 validation/rewrite-label-policy/run.py /var/tmp/NEW-dual --two-targets
```

Prerequisites are the preserved seed `/var/tmp/conglobate-cap-review-fix-normal-v1`,
clang with BPF target support, host runtime libbpf.so.1, and the unprivileged
libbpf header sysroot `/var/tmp/conglobate-cap-review-bpf/sysroot/usr/include`.
The runner compiles source afresh. These fixture paths are not deployment APIs.

Both recorded v1 runs pass on the seed Fedora kernel with SELinux boot-disabled:

- One target: the other writable ext4 mount denies label set/remove despite
  sharing its superblock owner namespace with the authorized mount.
- Two targets: that same second ext4 mount allows set/readback/remove.
- Both: exact-name and nonroot controls, unassigned tmpfs and foreign-namespace
  denials, capability checks, ordinary metadata operations, offline Fedora
  package installation, clean fsck and independent read-only remount checks pass.

`evidence/{single,dual}` preserves output, generated fixture scripts, source,
input manifest, launch command, verdict and provenance hashes. Larger artifacts
remain at the paths recorded in provenance. Earlier historical validation is
unchanged. An initial runner edit placed `if` after a semicolon and failed Python
parsing; it was corrected before either VM launched. The policy compiled without
warnings. Both successful runs use the same policy source/object.

Not tested here: Rust policy loading, partial installation/recovery, concurrent
builds, durable anchors, supervisor/guardian death, Arm, confidential hardware or
full distro image boots. No policy source activation is wired into conglobate yet.
The Rust map-description tests cover its native ABI and identity validation, not
kernel handle anchoring. M4 remains incomplete.
