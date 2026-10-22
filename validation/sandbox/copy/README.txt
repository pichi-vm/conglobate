Trusted COPY through the owning FUSE namespace
=============================================

Reproduce:
  python3 validation/sandbox/guest.py copy \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --out /tmp/NEW-COPY-FIXTURE --run

The safe-Rust helper links the repository's actual conglobate::recipe::Copy
implementation. It is a trusted worker-side process, never an image executable.
The guest uses a disposable 256MiB image in tmpfs, with no host shares/disks/NIC.

Sequence:
1. Trusted supervisor provisions the image and mapped-ID range. Worker enters
   the owning user/mount/network namespace and mounts a separate context tmpfs.
2. Trusted helper creates synthetic context data and metadata. Worker covers
   the context with a read-only bind mount. No context is placed in the image.
3. Worker mounts the image through fuse2fs, prepares command runtime files,
   and starts a command under the candidate entry helper/restrictions, in
   private mount/PID/network/IPC/UTS namespaces. The command verifies absence of
   context, worker helper, backing filesystem device and supervisor canary.
4. The command prepares destination links/account files and a bounded fixture
   writer (heartbeat plus atomic symlink replacement). The command exits;
   PID-namespace teardown kills its descendants. Worker checks stable heartbeat
   before invoking the real COPY implementation. There is no concurrent COPY.
5. COPY reads the read-only context from the trusted worker side. Its attempt
   to create a synthetic context file must return EROFS. Absolute source and
   destination links use the corresponding context/image root; the synthetic
   supervisor canary stays unchanged. Appended destination links are replaced,
   nested source links preserved, cycles and absent CARAPACE references rejected.
6. Named target UID/GID 1234:2345, mode 0751, nanosecond mtime, user xattr,
   explicit ACL bytes, independent SELinux label and file capability bytes are
   checked through FUSE. After worker exit and fsck, the initial-namespace kernel
   ext4 mount verifies the same on-disk metadata, including root-relative
   revision-2 file capabilities. ACL-byte preservation is not ACL enforcement
   or inheritance qualification; those known backend defects remain.

Bug reproduced and fixed:
  The original account lookup called read_to_string on the image's passwd/group
  path without a file-type check. A synthetic FIFO made it block until the
  trusted 1-second timeout killed the diagnostic helper. The prior result is in
  reproduced/copy-fifo-before-fix/. No output destination was produced.

  src/copy.rs now requires a regular file for account lookup before opening it.
  The same FIFO fixture returns an error promptly, inside the existing timeout.
  Unit coverage also rejects a link to FIFO, directory and pathname socket,
  while numeric ownership continues to bypass account databases. This check
  retains the stopped-writers prerequisite; it is not a race-resistant open API.

Setup observation:
  A filesystem remount using util-linux replayed tmpfs uid=100000 inside the
  mapped user namespace and failed (that ID is outside its range). Explicit
  uid=0/gid=0 did not cure the tested invocation. A bind remount with ro changes
  VFS mount flags without replaying tmpfs UID/GID options and passes EROFS.
  A future mount API implementation should set mount flags deliberately.

Evidence:
  reproduced/copy/{inputs.json,command.json,results.txt}: fixed implementation,
  complete metadata checks and ordinary kernel remount.
  reproduced/copy-fifo-before-fix/: prior timeout diagnostic; lacks the final
  expanded ACL/remount checks. Historical packaged inputs are individually hashed.
  cargo test --locked --lib copy::tests: 3 passed.
  cargo test --locked --test copy: 1 passed, 3 root tests initially ignored.
  Those 3 root tests then passed explicitly under sudo with a 30-second timeout.

Scope:
  This completes the bounded worker-side COPY prototype checks. Production
  launcher/worker integration must rerun them. No claim is made about executing
  COPY concurrently with recipe writers, universal filesystem semantics,
  export/publication, or ACL backend correctness.
