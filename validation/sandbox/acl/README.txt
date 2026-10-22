ACL diagnostic, 2026-10-20

Strict original reproducer: ../acl-worker and ../acl-comparison-results.txt.
The worker in this directory additionally prints the inode mode after setfacl
and retries the read after an explicit chmod 0640. Observed in initial and
mapped user namespaces:
  setfacl u:1234:r succeeds, getfacl reports named read grant;
  inode mode remains 0600; UID1234 read fails;
  chmod 0640; UID1234 read succeeds;
  newly created file lacks parent's default named-user ACL.

The diagnostic C interposer only logs the original fuse2fs init callback's
inputs/outputs; it does not change capabilities or filesystem behavior. It is
not production code and does not relax conglobate's forbid(unsafe_code) rule.
Compile against the headers from the recorded fuse3 source RPM:
  gcc -shared -fPIC -I. -I/path/to/fuse-3.18.3/include trace-init.c -ldl -o trace-init.so
Copy it to the disposable initramfs root, then use the worker with
LD_PRELOAD=/trace-init.so. Never inject it into a production build environment.
Generated public libfuse_config.h matches the recorded 3.18.3 build version.

IMPORTANT: the libfuse 3.18.3 debug INIT flags line is printed BEFORE the final
outarg.flags assignment. It cannot establish which flags went onto the wire.
An earlier investigation inference that ACL negotiation failed was withdrawn.
The callback logs show want changing from f819 to 8f819 (requesting ACL support).
This is consistent with the successful read after chmod. Actual end-to-end
permission and inheritance probes, not the debug flag line, establish failure.

Source inspection: fuse2fs op_setxattr stores the xattr and updates ctime; it
has no corresponding ACL-specific inode-mode update. This supports the observed
mode mismatch. It is not an exhaustive ACL implementation audit or a patch.

The debug session also repeats the independent hung-request abort experiment.
Guest serial output may interleave with sysrq reset text at the end; individual
probe results are retained and must be inspected rather than trusting QEMU exit.
