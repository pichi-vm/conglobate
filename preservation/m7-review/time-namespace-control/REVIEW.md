# Time namespace filter control

The maintained disposable-VM clone fixture now exercises CLONE_NEWTIME through
unshare. The legacy clone signal byte overlaps this flag, so adding it to the
legacy clone flag table would not test time-namespace creation. See primary API
documentation: https://man7.org/linux/man-pages/man2/unshare.2.html and
https://man7.org/linux/man-pages/man2/clone.2.html (accessed 2026-10-23).

Both arms retain effective CAP_SYS_ADMIN and use no_new_privs. The control must
successfully unshare and change /proc/self/ns/time_for_children identity. The
filtered arm uses the real CommandSyscalls filter and must return EPERM with
unchanged identity. No clock or offset is modified. The fixture only runs in its
disposable VM; all created namespaces vanish with its process. Existing ordinary
clone success and seven namespace-clone positive controls/denials remain required.

Native amd64 KVM test passed every required marker, exit status and VM shutdown.
C fixture compiled with -Wall -Wextra -Werror. No application Rust changed, so no
repeat of unrelated unit suites is claimed. Source comparison covers archived
application and fixture files. Raw evidence and source are retained; large assets
are hashed locally, not claimed remotely durable.

This closes the missing time-namespace mechanism coverage on the tested amd64
kernel. tests-001 remains partially qualified because arm64 execution is still
outstanding. It is not a full M4 adherence pass or general sandbox qualification.
