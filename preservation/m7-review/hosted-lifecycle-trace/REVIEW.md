# Detached workspace mount prerequisite

Diagnostic hosted run 40039132104 at 403275e fails in runtime-scope on Ubuntu
6.8.0-147-generic. The serial trace identifies open_tree on runtime-rYjg6U,
relative to the detached tmpfs descriptor, with OPEN_TREE_CLONE|OPEN_TREE_CLOEXEC,
returning EINVAL. The preceding fsopen/fsconfig/fsmount operations succeed.
Cache-storage and contained assessments pass; clone-filter was not reached.
Tracing is instrumentation, not uninstrumented qualification.

Upstream commit c5c12f871a302f6fae60f9f2840dad18adf049ce (preserved API response)
adds copying detached mounts from detached mounts, checking their origin mount
namespace. Its description specifically identifies exposing a subdirectory
without first exposing the whole filesystem as a use case. Source inspection
of fs/namespace.c at upstream v6.8 and v6.12 finds the old check_mnt rejection;
v6.17 has may_copy_tree/check_anonymous_mnt support. This establishes a concrete
capability prerequisite, not qualification based on a version number alone.

Source: https://github.com/torvalds/linux/commit/c5c12f871a302f6fae60f9f2840dad18adf049ce

The implementation retains its detached workspace design. CI now installs
Ubuntu 24.04 HWE instead of the GA kernel and removes diagnostic tracing for the
next run. Every case must still pass; no skip or fallback is introduced. The
new kernel has not yet been qualified by this change. No production Rust changed,
no adherence pass is awarded, and M4 remains 0/3.

All 159 regular files in each guest source archive match 403275e (the earlier
157 count described a selected subset). Logs, source and small artifacts are
preserved here. Kernel/initramfs assets remain local with hashes and finite
hosted retention; they are not durable M8 archives. The prior turn restated
verified networking status; this turn produces new source-grounded diagnosis
and changes CI to exercise the required mount capability.
