Paired tool/installroot runtime mount validation
===============================================
Scope: trusted stable root paths, stopped filesystem writers, private mapped
user/mount/PID/IPC/UTS/network namespaces. This is a prototype, not adoption in
the generic executor. The helper forbids unsafe Rust.

The helper opens both roots and their proc/dev/sys directories with O_PATH,
O_DIRECTORY and O_NOFOLLOW, retaining all six descriptors before the first
runtime mount. Mount destinations use the retained descriptors. Proc mounts are
nosuid,nodev,noexec; dev tmpfs is nosuid,noexec; empty sys tmpfs additionally is
read-only,nodev. Device admission remains the launcher's responsibility.

Reproduce (new output directory for each run):
 python3 validation/sandbox/guest.py runtime-pair \
   --kernel-version 7.2.10-200.fc44.x86_64 \
   --out /tmp/conglobate-runtime-pair-NEW --run
Retained inputs.json records input identities; command.json records the exact
isolated QEMU command. The compatibility run:
 python3 validation/sandbox/policy/package.py \
   --seed /tmp/conglobate-rebuilt-package-seed-v2 \
   --out /tmp/conglobate-fedora-pinned-runtime-NEW \
   --restricted-bootstrap --pinned-runtime \
   --protocol-worker-build /tmp/conglobate-statx-worker-v2 \
   --upgrade-rpms /tmp/conglobate-upgrade-rpms
The seed, RPM and worker identities are in retained inputs.json; these are
pre-existing validation fixtures, not network dependencies fetched by commands.

Results retained in reproduced/runtime-pair and reproduced/fedora-pinned-runtime:
* Valid pair: all six mount types and required flags checked from mountinfo.
* Eighteen negatives: each root/directory with symlink to an owned canary,
  regular file or absent target. All reject before any runtime mount, with
  ENOTDIR or ENOENT. Outer mountinfo and canary remain unchanged after each case.
* Fedora paired-FUSE restricted package fixture: DNF install, synthetic package
  upgrade, kernel-related fixture work, filesystem/remount checks, ownership,
  ACL, foreign SELinux label and capability persistence, worker-death detection
  and cleanup pass. Both guests exit zero. This run does not boot the output or
  perform a distribution-version upgrade.

Limits: root paths themselves are trusted; intermediate root components are not
an arbitrary-path traversal API. Concurrent rename/writer races are excluded by
the stopped-writer premise. Missing target creation/restoration is not tested;
the Fedora launcher creates directories only in its trusted fresh scaffolding.
An error after some successful mounts relies on private namespace teardown;
this matrix tests preflight rejection, not every partial-mount failure point.
No generic recipe launcher, runtime identity proof, new backend selection or
resolution of mapped-FUSE metadata limitations is claimed.

Retention: logs, QEMU commands, input catalogs, generated shell/Python scripts
and source snapshots are retained. retained-files.json binds source/generated
files; status-audit.json binds that catalog and primary logs. Large generated
VM images and RPMs are identified by the input catalog rather than duplicated.
