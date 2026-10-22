ACL correction experiment — partial remedy, capability semantics unresolved
===========================================================================

This modifies a disposable external fuse2fs 1.47.3 worker. It does not select
or install a production backend. Conglobate's safe-Rust requirement is unchanged.
acl.inc is GPL-2.0-only helper code inserted into the external GPL worker; it
uses libacl 2.4.0 for ACL validation and mode calculation.

Reproduction:
1. Produce a NEW worker directory with finalize/build.py using the command in
   ../finalize/README.txt. It must not already contain this ACL patch.
2. With libacl development headers/library available:
     python3 validation/sandbox/acl-fix/build.py --worker-build /tmp/NEW-WORKERS
3. Run:
     python3 validation/sandbox/guest.py acl-fix \
       --kernel-version 7.2.10-200.fc44.x86_64 \
       --worker-build /tmp/NEW-WORKERS --out /tmp/NEW-ACL-GUEST --run

The builder copies no binaries into the host installation. The guest has no
host shares, disks or NICs. Each case uses a new 64MiB tmpfs-backed ext4 image.
Tests compare kernel ext4 with initial-userns and mapped-userns FUSE, both with
and without metadata_csum. Every case is fsck-checked and remounted using kernel
ext4; final modes, IDs and ACL bytes must match the reference.

Corrections exercised:
- Validate ACL wire representation using libacl before storing it.
- Update inode permission bits when setting an access ACL.
- Update the access ACL mask/classes on chmod.
- Apply parent default ACL intersected with requested creation mode; preserve
  the default ACL on child directories. Apply umask only without a default ACL.
- Remove redundant three-entry access ACLs.
- Propagate mkdir ACLs even without metadata_csum.
- Include the primary GID in the worker's existing group-membership helper.

Passing common cases include named-user/group access and denial, chmod mask
revocation/restoration, inheritance under umask077 and explicit0600 creation,
grandchildren, hardlinks/rename, malformed-update rejection, ACL removal,
owner/nonowner authorization, and ordinary group-dependent SGID handling.

Two capability-sensitive cases still FAIL against kernel ext4:
  Caller                               Kernel       Experimental FUSE
  Nonroot + CAP_FSETID, outside group   Keep SGID    Clear SGID
  Root without CAP_FSETID, outside group Clear SGID   Keep SGID

Both differences reproduce in both FUSE namespace configurations and with both
filesystem feature choices. The probe verifies actual effective capabilities.
It restores a common mode AFTER reporting these differences so the separate
remount comparison remains useful. ACL_COMMON_SEMANTICS_MATRIX_PASS is explicitly
not a claim that all ACL semantics pass. guest.py requires the expected failure
counts as well as passing markers; a changed result requires investigation.

Why this remains unresolved:
The helper approximates privilege with UID/group checks. libfuse 3.18.3's
lib/fuse_lowlevel.c do_setxattr forwards arg->flags but not setxattr_flags, with
an explicit API-extension TODO. The protocol defines FUSE_SETXATTR_ACL_KILL_SGID,
but this high-level callback cannot use it to mirror the kernel decision.
UID0 is not a substitute for CAP_FSETID. A maintained capability-aware contract
or another backend is needed; proc-based capability guessing is not adopted.

Evidence:
  ../reproduced/acl-fix/{inputs.json,command.json,results.txt}
  ../reproduced/acl-fix/acl-build-inputs.json
  acl.patch (applies on top of the experimental finalization patch)

This is bounded differential evidence, not qualification of every ACL shape,
concurrent operation, failure path or OS build. Device creation, trusted.*
xattrs, timestamps/file flags, and the full publication path have separate gates.

Follow-up: ../acl-protocol/README.txt demonstrates a paired-library accessor
for the kernel decision. Its six-way matrix corrects both capability mismatches
without changing command privileges. The original acl-fix case remains an
explicit negative control; stock libfuse has not been modified or adopted.
