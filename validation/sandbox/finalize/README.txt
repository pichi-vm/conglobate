Finalization error reporting — experimental backend correction
=============================================================

This is a disposable external-worker experiment, not selection of a backend
or a production conglobate change. Conglobate Rust still forbids unsafe code.
The patch modifies the existing C fuse2fs tool under its applicable GPL terms;
the diagnostic wrapper is linked only into separately named test binaries.
No binaries are installed on the host.

Build from extracted e2fsprogs 1.47.3 and libfuse 3.18.3 source packages:
  python3 validation/sandbox/finalize/build.py \
    --source /tmp/conglobate-acl-source/e2fs/e2fsprogs-1.47.3 \
    --fuse-headers /tmp/conglobate-acl-source/fuse/fuse-3.18.3/include \
    --fuse-config /tmp/conglobate-acl-source/libfuse_config.h \
    --fuse-library /usr/lib64/libfuse3.so.4 \
    --out /tmp/NEW-FINALIZATION-WORKERS

Then:
  python3 validation/sandbox/guest.py finalize \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/NEW-FINALIZATION-WORKERS \
    --out /tmp/NEW-FINALIZATION-GUEST --run

The source C file's baseline SHA256 is asserted by the builder. Sources are
copied before patching. Configuration/build logs, generated patch, input hashes
and four separately named workers are retained in the worker output directory.
The libfuse config header declares 3.18.3 and versioned symbols; see the earlier
ACL source-package fixture. The build requires a compiler/make and the runtime
libfuse library, but does not install development packages.

Experimental patch:
- Add an atomic sticky error flag to the worker context.
- Set it when the existing error translator classifies a filesystem error as
  serious (its existing is_err classification).
- Set it on any ext2fs_close failure.
- Return failure status 32 if the flag was ever set, even if the final close
  later succeeds. Clean completion still returns 0.

Fault matrix observed in a guest-only 64MiB image:
  Case                                      Baseline     Patched
  Clean writes and unmount                       0           0
  Diagnostic close-only EIO                      0          32
  Real dm-error during final flush/close          0          32
  Real fsync EIO, then device restored          not run      32

The close-only wrapper calls the real close successfully, then returns EIO.
This isolates propagation of a final close failure, not corruption of media.
Clean and close-only images pass e2fsck. The real device-error cases use a
linear device-mapper target switched to error by the trusted guest supervisor.
In the recovered case dd's fsync fails first, then the original linear target
is restored; no later closing error is printed, but sticky status remains 32.
The guest has no host shares, host disks or network interfaces.

Evidence:
  reproduced/finalize/results.txt, inputs.json, command.json
  reproduced/finalize/build-inputs.json
  finalization.patch

The first retained build manifest was augmented after the build with complete
source/header hashes; its original builder-script hash is preserved. The current
build script records those source/header hashes directly for future runs.
Guest inputs.json hashes every packaged worker executable and library.

Limits and required follow-up:
- This resolves the reproduced final-flush and close-only exit-status defect
  in the experimental worker. It does not prove every backend error path is
  classified correctly, nor does it qualify a maintained production backend.
- All recipe descendants must still be gone before worker finalization. The
  supervisor must reject failed worker status before sealing/hashing/publishing.
  The complete artifact/inventory/cancellation sequence remains item 16.
- ACL mode consistency/inheritance, device/trusted.* limitations, filesystem
  compatibility and dependency maintenance are not fixed by this patch.
- No change to the installed fuse2fs or the production executor was made.

Upstream source check:
  An unmodified shallow checkout of e2fsprogs commit
  8fd79523d051d5ea881237f27eb1c664486e0078 was inspected. Its op_setxattr still
  stores ACL xattrs without an ACL-specific mode update; its unmount close
  routine still logs close errors without returning them. No claim is made
  that merely upgrading fixes either reproduced blocker.
  https://github.com/tytso/e2fsprogs/blob/8fd79523d051d5ea881237f27eb1c664486e0078/misc/fuse2fs.c

Release-callback error follow-up (2026-10-21 UTC)
------------------------------------------------
Diagnostic EIO after a successful release flush is sticky: command write returns
success, worker exits32. A full authenticated FUSE build now rejects that worker
status before sealing/publication, preserves base/mountinfo and releases FUSE/
DM/loops. Control exits0 and both standalone files retain correct content.
Earlier integrated case failed a later write with EROFS and is separately
retained; final last-write case isolates worker-status rejection. See validation/
sandbox/release-finalize. This does not qualify every error class or completion
protocol, nor adopt the experimental backend.
