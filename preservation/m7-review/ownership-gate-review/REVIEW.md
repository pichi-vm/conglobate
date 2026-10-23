# Ownership admission gate: focused review

This is a focused change review, not a complete milestone adherence pass.

- Requirement: a command must not receive a view that silently substitutes
  overflow owners for inherited unmapped IDs. The new preparation gate checks
  original filesystem IDs against the actual selected UID/GID range counts.
  Allocation policy is deliberately still an open requirement.
- Authority: validation borrows the mounted volume's pinned root after checking
  its namespace scope. The supervisor owns a quiescent filesystem at this point;
  no image commands run concurrently. Both bootstrap roots are checked.
- Traversal: O_PATH/NOFOLLOW observations do not activate special files.
  Directory descent is descriptor-relative with NO_XDEV and NO_SYMLINKS. Image
  symlinks are checked for their own ownership but never followed. ACL reads
  use trusted supervisor procfs to resolve a live pinned descriptor; no command
  filesystem path determines the procfs path. This relies on the existing
  supervisor procfs and private-mount assumptions.
- Metadata: named user/group entries in access and default ACLs use independent
  range checks. Non-named tags require the undefined ID. Unsupported/failed ACL
  reads fail, except absent attributes. The parser validates encoding and IDs;
  the kernel supplies POSIX ACL structure/order semantics. Linux symlinks have
  no POSIX ACLs and are skipped only for that read, not ownership checks.
- Lifetime/error paths: immutable policy, borrowed root, owned traversal frames
  and RAII descriptors; validation returns no reusable mutable receipt. Failure
  enters existing EmptyDrain recovery with namespace and volume owners retained.
  The original absolute setup deadline is checked throughout. Kernel-blocked
  metadata syscalls are subject to the existing VM disposal limitation.
- Tests: 128 library tests; native amd64 VM inode rejection, ACL-only rejection,
  rollback and positive two-root bootstrap/export. Strict all-target Clippy
  initially rejected a remainder-check idiom; corrected check passed. Raw logs,
  source archives, and differences from tested revisions are retained.
- Remaining coverage: high GID and default ACL VM cases, special files on ext4,
  expired scans/large directory trees, and the supported range/allocation
  contract. These are not silently promoted to passed. Current per-command
  scans also need qualification against larger distro build workloads.

M4 remains 0/3. No broader sandbox, distro boot, arm64 or confidential platform
qualification follows from this review.
