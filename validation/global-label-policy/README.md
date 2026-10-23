# Registration-free labeling mechanism

This validation constructs a disconnected native amd64/KVM guest using trusted
host tools. It never loads BPF, mounts filesystems, or changes credentials on the
host. Run with the established external scratch directory:

```
TMPDIR=/var/tmp/conglobate-rewrite-tmp python3 validation/global-label-policy/run.py NEW_OUTPUT \
  --kernel /boot/vmlinuz-7.2.10-200.fc44.x86_64 \
  --kernel-release 7.2.10-200.fc44.x86_64 \
  --headers /var/tmp/conglobate-cap-review-bpf/sysroot/usr/include
```

The VM loads one exact-name skipcap hook, pins it, and exits the loader. There
are no maps, filesystem registrations, subject registrations or activation
updates. Commands run with namespace UID 0 mapped to outer UID 65536 and verify
zero effective capabilities. Before loading and after removal, labeling fails.
With the policy loaded, foreign SELinux label set/read/remove succeeds on ext4;
read-only bind mounts reject writes; security.capability, security.other,
security.selinux.extra and trusted.fixture receive no exception.

The positive mode also confirms that ordinary file mode 0444 does not prevent
label changes: security.* attribute permission checks differ from file writes.
Reachability and read-only views remain necessary integrity boundaries.

`preservation/m7-review/global-label-v2` records the passing run. This is a
mechanism probe, not the production command sandbox: it deliberately sees the
fixture runtime, uses a one-UID mapping and does not exercise idmapped mounts,
bootstrap dual roots, seccomp, network isolation, process draining or recovery.
Those need revalidation after the production lifecycle is simplified. The
production binary still uses the scoped policy. This result does not establish
Arm or confidential-platform qualification.
