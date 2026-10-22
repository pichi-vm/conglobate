Restricted initial bootstrap fixture
====================================
Reproduce after building package-fixture seed, package-upgrade RPMs and the
acl-protocol pair (see their READMEs):

python3 validation/sandbox/policy/package.py \
  --seed /tmp/conglobate-rebuilt-package-seed-v2 \
  --out /tmp/conglobate-restricted-bootstrap-NEW \
  --upgrade-rpms /tmp/conglobate-upgrade-rpms \
  --protocol-worker-build /tmp/conglobate-acl-protocol-v5 \
  --restricted-bootstrap

The output directory must not exist. QEMU has no NIC or host filesystem share
and a 150-second timeout. All guest workspace and block storage is RAM-backed.
The guest disables SELinux; host policy is unchanged.

The launcher creates a private tool root, binds synthetic guest /usr, /etc and
/rpms read-only, mounts the writable target FUSE filesystem at /output, adds
private proc/dev and an empty read-only sys directory, then invokes the same
restricted entry helper as other command tests. It pivots/detaches old root,
closes inherited descriptors and sets a clean environment. Commands assert
PID1, no_new_privs, seccomp and absence of supervisor/worker paths. The worker
runs outside the command PID/mount namespaces and shares the mapped userns.
This is a fixture tool root, not an authenticated tool-carapace implementation.

The initial offline DNF transaction passes under restrictions, then the existing
restricted attr reinstall/remove/install and version-changing synthetic upgrade
run in the new root. Strict ACL assertions, fsck, kernel remount, installed shell
and unchanged-origin checks pass. See ../reproduced/restricted-bootstrap for
logs, packaged input hashes, selected sources and paired-worker build catalog.
The negative mount-order fixture demonstrates why the root bind must precede
child mounts: a later nonrecursive bind hides tools and exec returns ENOENT.

Scope: this pinned package set is not a whole OS or kernel/initramfs build.
Network is disabled and packages explicitly supplied; signature checks are off.
No claim of a production input/attestation chain is made. The fixture's later
crash-worker is stock fuse2fs, so its markers do not qualify the corrected pair.
Mapped mknod/trusted-xattr/immutable limitations remain independently recorded.
