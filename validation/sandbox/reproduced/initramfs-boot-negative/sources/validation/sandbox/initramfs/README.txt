Restricted distribution initramfs workload (partial evidence)
=================================================================

This extends the restricted-bootstrap package fixture with Fedora kernel-core,
kernel-modules-core and dracut plus their downloaded dependency closure. The
initial guest kernel stays 7.2.10-200.fc44; the output kernel package is
7.2.11-200.fc44. The older matching kernel RPM was unavailable in the configured
repository. rpms.json pins the new 183-package input set; it is an explicit
separate fixture, not a silent replacement of the original bootstrap packages.

Download origin command (versions available on 2026-10-20):
  dnf5 download --resolve --alldeps --destdir=/tmp/NEW-INITRAMFS-RPMS \
    dracut kernel-core kernel-modules-core
For reproduction, request pinned NEVRAs from rpms.json and verify SHA256; current
repositories may no longer serve them. No signatures or reproducibility claims
are inferred from the catalog. Downloads occur outside the guest; the guest
runs offline, with no host share or host disk attached.

Run:
  python3 validation/sandbox/policy/package.py \
    --seed /tmp/conglobate-rebuilt-package-seed-v2 \
    --out /tmp/NEW-INITRAMFS-GUEST \
    --upgrade-rpms /tmp/conglobate-upgrade-rpms \
    --protocol-worker-build /tmp/conglobate-acl-protocol-v5 \
    --restricted-bootstrap --initramfs-rpms /tmp/NEW-INITRAMFS-RPMS

The runner binds additional RPM inputs read-only into the bootstrap tool root
and installs them through restricted DNF. The later command runs ordinary
non-hostonly dracut in the generated target filesystem, checks the archive
listing for systemd and kernel modules, and records an image hash. The trusted
kernel-remount phase checks that hash after filesystem-worker teardown.
This mode has 8GiB guest RAM and a 300-second VM timeout. Production code and
command restrictions are unchanged. Success is not bootability, full OS build,
source-kernel compilation or attested input/output qualification.

Results must be inspected for package scriptlet failures independently of DNF's
exit status. No result is claimed here until a completed retained run exists.

First completed run
-------------------
Evidence: ../reproduced/initramfs-first. DNF transactions complete, restricted
dracut emits an archive with systemd/module entries, and its hash verifies after
worker teardown and kernel remount. fsck and existing package checks pass.
This is partial evidence, not a clean compatibility pass: dracut reports a
missing /dev/fd process-substitution path, absent sysfs firmware-path control,
and unavailable initramfs syslog logging. Package scriptlets also emit missing
proc/sys, systemd catalog/preset, and audit warnings. These need classification
and runtime-mount corrections before treating the workload as qualified.

A long-lived detached test child finishes its finite loop during this larger
workload and falls through to its parent's waitpid code, producing ECHILD.
This is a fixture control-flow bug; the ordinary workload ended before it was
observable. Its eventual exception is not proof of sandbox failure. Fix the
fixture and retain an explicit descendant-liveness check in the rerun.
No boot test, source-kernel build or general clean-install claim is made.

Runtime-mount and long-workload follow-up
---------------------------------------
Evidence: reproduced/initramfs-runtime (relative to validation/sandbox). The
bootstrap tool and installroot now each have private proc/dev mounts and an
empty read-only sys view. Device descriptor/stdin/stdout/stderr links refer to
the command's proc, not supervisor descriptors. The later command receives
these links too. Missing-proc/catalog and /dev/fd diagnostics disappear; dracut
generation and post-teardown fsck/kernel-remount hash checks pass.

Detached children now exit explicitly when their bounded loop ends. Both
heartbeat probes and trusted canary have bounds beyond the300-second guest
timeout; commands require both heartbeats to grow immediately before exiting.
Subsequent stable-heartbeat checks therefore exercise termination, not natural
expiry. The rerun has no traceback and passes liveness and teardown assertions.

The runner now checks the exact initramfs RPM set and SHA256 catalog before
creating output. Bounded missing/corrupt input tests confirm early rejection.
Package-source evidence shows systemd-udev's script references the missing
systemd-tmpfiles-clear.service; neither systemd nor systemd-udev ships it. The
audit warning is consistent with the candidate policy rejecting audit netlink.
Remaining dracut warnings concern absent firmware sysfs discovery and syslog;
local dracut source records their cause. Explicit target firmware directory
and stderr-only logger configuration are available follow-ups. No VM logging
service or host sysfs exposure is required by that source analysis. Boot testing
and source-kernel compilation remain open; no complete OS qualification claim.
