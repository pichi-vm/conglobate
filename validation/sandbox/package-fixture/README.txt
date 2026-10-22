Repository-based assembly of the Fedora package workload
========================================================

The old /tmp/conglobate-fuse-adversarial seed is no longer required.

1. Provision the 69 RPM files listed in rpms.json into a package directory.
   Filenames pin versions/architectures; SHA256 pins the exact bytes. Existing
   downloaded packages can be reused. On a Fedora host with repositories still
   serving those versions, a download-only invocation can be constructed as:

     python3 - /tmp/NEW-RPM-DIRECTORY <<'PY'
     import json, pathlib, subprocess, sys
     out = pathlib.Path(sys.argv[1]); out.mkdir()
     names = json.loads(pathlib.Path('validation/sandbox/package-fixture/rpms.json').read_text())
     subprocess.run(['dnf5', 'download', '--destdir', str(out),
                     *(name.removesuffix('.rpm') for name in names)], check=True)
     PY

   Historical package availability is external. Never substitute newer bytes
   silently: the builder rejects missing/mismatched packages. This is a fixture
   input lock, not the production refs.lock or a replacement for signature policy.

2. Assemble a fresh seed from repository scripts and explicit host tools:

     python3 validation/sandbox/package-fixture/build.py \
       --kernel-version 7.2.10-200.fc44.x86_64 \
       --rpms /path/to/RPM-DIRECTORY --out /tmp/NEW-PACKAGE-SEED

3. Build the optional two-version RPM fixture using ../package-upgrade/README.txt,
   then run the current restricted-command workload:

     python3 validation/sandbox/policy/package.py \
       --seed /tmp/NEW-PACKAGE-SEED --upgrade-rpms /tmp/NEW-UPGRADE-RPMS \
       --out /tmp/NEW-PACKAGE-GUEST

The seed builder does not run a guest itself. The policy wrapper installs the
current safe-Rust entry helper and restriction assertions before execution.
The eight retained scripts originate from the earlier workload; its heartbeat
loops now have finite bounds. Known ACL diagnostics still report failures.

Inputs and host requirements:
- Fedora-compatible command binaries enumerated in build.py, their ldd runtime
  dependencies, /usr/lib/rpm support files, selected kernel and module closure.
- No root/mount privileges are used to assemble the archive.
- No host repository definitions, credentials, user configs, policy binaries or
  old seed contents are copied. A synthetic disabled file:// repository satisfies
  DNF's --disable-repo=* matching, and /etc/mtab points to /proc/mounts.
- seed-inputs.json records every copied regular file/symlink and source script.
  The policy wrapper additionally hashes the final guest payload and command.
- Host tool versions may differ in a new environment; these hashes make that
  difference visible. This is repeatable assembly, not a claim of bit-identical
  output across arbitrary tool/kernel versions.

Recorded validation:
The freshly assembled seed passes the existing restricted package/isolation/
descendant/failure checks plus the version-changing config/scriptlet transaction
and kernel remount. Initial full Fedora installation still precedes restrictions.
The original ACL failures remain. No host shares, disks or NICs are exposed.

Negative input check:
  python3 validation/sandbox/package-fixture/check-inputs.py \
    --kernel-version 7.2.10-200.fc44.x86_64 --rpms /path/to/RPM-DIRECTORY
This modifies only a disposable package copy and asserts rejection before the
output directory exists.

Evidence: ../reproduced/package-rebuilt/
  inputs.json, command.json, results.txt, seed-inputs.json, input-check.txt

This completes removal of the opaque seed dependency, not full OS/kernel build
qualification, performance analysis or the dependency/license maintenance audit.
