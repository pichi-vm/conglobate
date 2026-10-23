# Recipe-command labeling on both bootstrap filesystems

Use a local, verified Fedora tool image containing attr's setfattr/getfattr.
Run this recipe through `tests/distro/run.py` with matching reference and index
pin, then run `verify.py RUN_DIRECTORY` (Python cbor2 required). No recipe step
uses networking.

Commands set/read foreign security.selinux values on `/` and `/mnt`; values
survive the next step's remount and can be removed and set again. The exact-name
boundary is checked by denying security.selinux.extra, security.other and
trusted.conglobate. Label writes to the private read-only `/sys` must fail EROFS.
These checks exercise image binaries after command admission and restrictions.

This is not a differential test of the two pre-admission probes: deleting one
probe could still leave command-level label operations working. That separate
coverage obligation remains open. This minimal output is not a bootable distro,
and no complete sandbox or confidential-platform qualification is claimed.
