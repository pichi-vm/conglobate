# Trusted guest labeling fixture

These two files provision the fixed exact-name `security.selinux` exception in
an owned disposable VM. They are not linked into conglobate, not a recipe input,
and must never be loaded on the development host. The loader requires the guest
marker, loads once and pins the link for the VM lifetime. There are no subject or
filesystem registrations. Ordinary inode, mount, ownership and ACL checks remain.

The production execution environment must supply the equivalent contract.
The fixture requires clang with BPF support, a C compiler and libbpf development
files. `label_environment.py` obtains flags from pkg-config. Explicit
`CONGLOBATE_TEST_BPF_CFLAGS` and `CONGLOBATE_TEST_BPF_LIBS` override those flags
for an externally prepared sysroot; they contain shell-style argument lists,
parsed as arguments and never executed as shell code. Multiarch UAPI headers are
found using the C compiler's `-print-multiarch` result when present.
