Metadata fidelity and file-flag differential probes
==================================================

Build the experimental worker as described in ../acl-fix/README.txt, then:
  python3 validation/sandbox/guest.py metadata \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/NEW-WORKERS --out /tmp/NEW-METADATA-GUEST --run

Six independent 64MiB tmpfs images compare kernel ext4, initial-userns FUSE and
mapped-userns FUSE with/without metadata_csum. The guest has no host shares,
disks or NICs. Flags are applied only to disposable regular files; the device
node is only created/stat'ed, never opened. Restrictive file flags are cleared
before unmount. These are backend semantics probes, not the restricted-command
launcher; final capability/filter policy may further restrict operations.

Observed results (same with/without metadata_csum):
  Operation                       Kernel / initial FUSE     Mapped FUSE
  ns times before epoch/2038+      Exact                     Exact
  Symlink timestamps              Persist                   Persist
  user.* and foreign SELinux label Store/persist             Store/persist
  nodump flag                     Set/persist               Set/persist
  trusted.*                       Store/persist             EPERM
  Character device metadata       Create/persist            EPERM
  immutable flag                  Set/enforce/clear         EPERM on set
  append-only flag                Set/enforce/clear         EPERM on set

Initial-userns FUSE matches the kernel reference for the original metadata
operations. The extent follow-up below records additional differences.
Immutable denies ordinary/append writes and chmod. Append-only permits append
but denies nonappend writes and chmod. The mapped cases cannot test enforcement
of newly set restrictive flags because setting them fails.

All six images pass fsck and exact comparisons of recorded metadata after a
kernel read-only remount. Only explicitly set timestamps are compared; ctime,
birthtime and automatically updated timestamps are not covered. This is not a
complete flag/xattr matrix, idmapped-mount test or proof that every package
workflow is compatible. SELinux is disabled in this guest, not on the host.

guest.py checks six observation/remount records, exact timestamp values, the
passing xattr/nodump operations and the mapped EPERM boundaries. Diagnostic
reference differences are retained; METADATA_MATRIX_COMPLETE means the matrix
finished, not that kernel and mapped-FUSE semantics are equivalent.

Evidence: ../reproduced/metadata/{inputs.json,command.json,results.txt}
Worker patch/build identity: ../reproduced/acl-fix/acl-build-inputs.json
The x86_64 ioctl numbers derive from the host linux/fs.h FS_IOC_GETFLAGS and
FS_IOC_SETFLAGS definitions; this fixture intentionally supports only x86_64.

Design consequence: granting namespace root alone does not give a complete
real-root image-preparation experience. Immutable/append-only metadata joins
trusted.* and device-node creation among the unresolved compatibility gaps.
This probe does not establish which kernel/worker boundary causes each denial,
nor select a workaround. Do not silently omit this metadata in produced images.

File-capability follow-up
-------------------------
Evidence: ../reproduced/capability-metadata. All six configurations now also
exercise security.capability on four disposable regular files of mode0600.
The revision2 value has CAP_NET_BIND_SERVICE in permitted, effective set,
no inheritable bits. No capability-bearing file is executed or made executable.

Every backend accepts the exact 20-byte revision2 value and preserves it after
kernel remount. The mapped FUSE case does not leave a revision3 host-UID rootid
in the on-disk value. Ordinary content replacement and chown to1234:2345 clear
the xattr (ENODATA); chmod to0640 preserves it. Kernel ext4, initial FUSE and
mapped FUSE agree with and without metadata_csum. The runner asserts exact
setup/operation/results for every case, plus the original fsck and exact
kernel-remount checks. Existing mapped trusted.*/device/flag limitations remain.

Constants are from linux/capability.h: VFS_CAP_REVISION_2=0x02000000,
VFS_CAP_FLAGS_EFFECTIVE=1, CAP_NET_BIND_SERVICE=10. The checked local header's
SHA256 is retained. This does not test execution/privilege acquisition, all
capability bits, revision3 namespaced-capability workflows, or the final command
restriction policy. It establishes this useful on-disk metadata path only.

Inherited restrictive flags and kernel permission boundary
----------------------------------------------------------
Evidence: ../reproduced/inherited-flags. Before every backend run, initial root
seeds two disposable files through kernel ext4 with immutable/append-only bits.
Kernel ext4 and initial-userns FUSE can clear and restore each bit. Mapped FUSE
can read the flags but clearing returns EPERM. Kernel remount confirms both
bits remain set in all six cases, with/without metadata_csum. Thus the issue
also affects updating inherited files, not merely authoring new flagged files.

Source analysis, pinned upstream revision:
  f47d223627f80cb7a1eb14c585fe6cb35de5e984
  kernel-source/sources.json records URLs and SHA256 for the four source files.
fs/ioctl.c routes SETFLAGS through the generic attribute path. In
fs/file_attr.c, fileattr_set_prepare requires CAP_LINUX_IMMUTABLE when either
restrictive bit changes; vfs_fileattr_set applies this before the filesystem
callback. kernel/capability.c defines capable against init_user_ns. FUSE's
fileattr callback subsequently forwards the ioctl to userspace.

This explains why child-userns capabilities and worker-side changes do not
satisfy that pre-dispatch check. This is upstream source corroboration, not an
exact Fedora kernel build-source audit. The guest result independently records
actual behavior. No kernel patch, new privileged broker or alternate metadata
API is adopted here. These would change the design and need separate trust
analysis; silently omitting these bits is not acceptable.

Bounded allocation/extent follow-up
----------------------------------
Evidence: ../reproduced/extent-metadata. Same six configurations and invocation
as above, with acl-patched from the recorded worker build (not the later paired
libfuse protocol worker). Local headers and matching worker source are retained
with hashes; op_fallocate explicitly rejects unsupported flags such as collapse.
The initial exploratory harness rejected the new kernel/FUSE differences. The
updated harness keeps the original metadata comparison and separately asserts
the exact extent outcomes below; it does not call these differences passes.

Each operation uses a fresh 64KiB regular file containing four distinct 16KiB
regions. Only allocation extends to 128KiB. Successful operations must produce
exact expected bytes; failed operations must leave bytes unchanged. fsync,
fsck and kernel read-only remount follow, checking size and SHA256 persistence.

                             Kernel ext4          Initial/mapped FUSE
Allocation beyond EOF        succeeds             succeeds
Punch aligned middle range   succeeds             succeeds
Zero aligned middle range    succeeds             succeeds
Collapse aligned middle      succeeds/shrinks     ENOTSUP/unchanged
FICLONE same-filesystem       ENOTSUP/unchanged     ENOTSUP/unchanged
Seek data from punched hole  next data (32768)    same offset (16384)
Seek hole from punched hole  same offset (16384)  EOF (65536)

Both metadata_csum settings have these results. Reflink rejection is shared
with this kernel ext4 reference, not unique to FUSE. Sparse seek differences
mean consumers cannot rely on discovering punched holes through this mount;
content is correct but sparse-aware copy efficiency may differ. This test does
not measure allocation reduction, unaligned operations, extent fragmentation,
ENOSPC, reflink fallback by packages, or behavior through final command policy.
Range collapse is a real worker compatibility limitation. It must not silently
succeed or be described as a root-equivalent filesystem operation. Broader
filesystem and representative OS workloads remain required.

Unaligned ranges, allocation accounting and corrected pair
----------------------------------------------------------
Evidence: ../reproduced/unaligned-extents and unaligned-extents-paired. The
metadata case now optionally accepts --protocol-worker-build pointing to the
acl-protocol pair, alongside --worker-build. The paired run uses version5 from
the retained build catalog; packaged worker and library bytes were verified.
Both old common-ACL worker and corrected pair pass the same six configurations.

New range cases start at offset123, length9001 (crossing block edges), offset123
length19 (one block), and offset65500 length9001 (past EOF). Punch and zero both
preserve exact bytes outside their range. Punch keeps file size; zero extends
it when necessary. All 36 range cases per run pass fsync, fsck and kernel-remount
size/content checks. These are backend tests, not final command-policy tests.

Recorded st_blocks now persists identically after kernel remount. For the aligned
64KiB fixture, allocating another64KiB increases st_blocks by128 sectors; punching
16KiB decreases it by32 sectors; zeroing leaves it unchanged. Kernel collapse
decreases it by32; rejected FUSE collapse leaves it unchanged. Thus the tested
punch frees blocks even though FUSE SEEK_HOLE does not expose the hole. This
confirms allocation accounting for these cases, not arbitrary fragmentation or
large-image efficiency. The harness asserts these deltas and every range result.

The corrected pair retains the previously recorded collapse/reflink/sparse-seek
limitations and mapped mknod/trusted-xattr/immutable restrictions. No attempt is
made to hide those differences. Earlier metadata results remain bounded to their
recorded workers. ENOSPC and broader workloads are separate validation gates.
