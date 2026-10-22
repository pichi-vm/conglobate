Birth-time worker prototype validated — 2026-10-21 UTC
====================================================
Build:
 python3 validation/sandbox/statx-worker/build.py \
  --pair /tmp/conglobate-clean-pair-v2 --out NEW-WORKER
Test:
 python3 validation/sandbox/guest.py metadata \
  --kernel-version 7.2.10-200.fc44.x86_64 \
  --worker-build /tmp/conglobate-finalize-workers \
  --protocol-worker-build NEW-WORKER --expect-birthtime --out NEW-GUEST --run
The build uses a separate copy of the clean paired worker source, a fresh
configure/build directory and the existing paired libfuse. No original worker,
installed library, conglobate production code or host filesystem mount changed.
Exact patch/source/build logs and hashes retained in reproduced/statx-worker-build;
actual guest inputs/source/results in reproduced/statx-metadata.

The callback resolves pathname or existing file handle and obtains all stat
fields and i_crtime under the existing worker lock. Its helper shares the inode
read with getattr; only statx additionally requests birth time. Returned mask
includes BTIME only when i_crtime fits the inode. Linux basic stat fields are
mapped to statx, and no unsupported optional attribute masks are advertised.
This is a pinned Linux/libfuse3 diagnostic implementation, not a portable patch.

All six configurations now expose birth time, preserve it through write/chmod/
rename/read, and match it exactly after fsck/kernel remount. Open-file query,
open-file query after unlink, absent pathname failure and dangling-symlink query
also pass six times. Previous54 automatic update checks remain passing. The full
existing metadata, extent, capability and filesystem-ENOSPC assertions also pass;
known privileged-metadata and extent limitations remain unchanged. Original
filesystem-enospc log also passes the updated validator in baseline mode.

This demonstrates a practical correction of birth-time visibility for the tested
regular-file/symlink/open-file paths without new kernel privileges or a FUSE
protocol change. It does not solve mapped trusted.* / mknod / immutable flags.
Remaining qualification includes old/small inode formats without crtime, broader
optional statx fields and concurrent mutation, platform/configure guards, and a
maintained backend adoption path. Production security and general Linux-statx
conformance are not implied. No source was submitted upstream or runtime selected.
The first source-copy attempt failed on temporary storage quota; the successful
fresh build/test was run after retaining prior evidence and freeing only generated
files. No failed runtime result was discarded or treated as a pass.
