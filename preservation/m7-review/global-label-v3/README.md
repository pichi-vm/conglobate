# Anonymous admission probe mechanism

Extends v2 with O_TMPFILE|O_EXCL|O_CLOEXEC on assigned ext4, followed by
fsetxattr/fgetxattr/fremovexattr and close, under zero effective capabilities.
ANONYMOUS_LABEL_PROBE_PASS is required in addition to the prior boundaries.
The guest changes the assigned root directory owner to outer UID 65536 so
namespace root can create the inode. This is not an idmapped-mount test.
No named probe file is created. Ext4 allocation/journal effects are not claimed
absent. Production safe-Rust implementation, full command seccomp, dual roots,
and admission-failure cleanup remain to be validated.
