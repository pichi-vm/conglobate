Version-changing RPM upgrade under the restricted FUSE command
=============================================================

Build two synthetic packages:
  python3 validation/sandbox/package-upgrade/build.py \
    --rpmbuild /path/to/rpmbuild --out /tmp/NEW-UPGRADE-RPMS

Run the retained Fedora fixture with the additional transaction:
  python3 validation/sandbox/policy/package.py \
    --seed /tmp/conglobate-fuse-adversarial \
    --upgrade-rpms /tmp/NEW-UPGRADE-RPMS --out /tmp/NEW-UPGRADE-GUEST

The recorded initial run used the earlier local seed. A repository-based seed
builder is now available in ../package-fixture/README.txt, and the upgrade checks
have also passed using a freshly assembled seed.

Tool used for the recorded run:
  rpm-build-6.0.2-1.fc44.x86_64.rpm, downloaded using dnf5 download.
  rpmkeys --checksig reported digests/signatures OK.
  rpm2cpio/cpio extracted it into a disposable directory; it was not installed
  on the host. Existing host rpm-build-libs 6.0.2 supplied its runtime library.
  The command can also use an already installed compatible rpmbuild.

fixture.spec creates versions1 and2 of a data-only package. Scriptlets write
only fixture paths inside the installed root. No compiler or arbitrary network
download is involved. The builder records spec/tool/RPM hashes and logs.
SOURCE_DATE_EPOCH and buildhost are fixed, but bit-for-bit reproduction across
host RPM macro/tool versions is not claimed. The current extracted-tool setup
emits changelog/ELF-macro warnings; both data-only builds complete successfully.
Generated RPMs are intentionally unsigned test inputs and hashed in the guest
manifest. Signature skipping here is not a production input-authentication rule.

Inside the restricted command on mapped-userns FUSE:
- Install version1 and assert the RPM database and payload version.
- Edit the packaged %config(noreplace) file.
- Upgrade to version2 and assert both database/payload changed.
- Assert the local config survives and version2's default appears as .rpmnew.
- Assert exact scriptlet order/arguments:
    pre-1-1, post-1-1, pre-2-2, post-2-2, preun-1-1, postun-1-1.
- Assert the scriptlet-created state file records mode0640/UID1234/GID2345.

After command teardown and worker unmount, kernel ext4 remount verifies the
version2 payload, both config files and state-file metadata. Existing fixture
isolation, descendant cleanup and attr reinstall/removal tests also pass.
The old ACL diagnostic failures remain and are not counted as passes.

Evidence:
  ../reproduced/package-upgrade/{inputs.json,command.json,results.txt}
  build-inputs.json, additional-inputs.json, build-1.log, build-2.log

Limits:
The full initial Fedora package installation still occurs before command
restrictions. These are synthetic version-changing RPM transactions using
--nodeps, not a solver-driven distribution upgrade, kernel/initramfs build,
service-heavy package qualification, performance measurement or full OS build.
No production executor or backend adoption follows from this result.
