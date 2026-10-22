Runtime mount target path validation: bounded negative control
==============================================================
Run:
  python3 validation/sandbox/guest.py runtime-path --kernel-version 7.2.10-200.fc44.x86_64 --out /tmp/NEW-runtime-path --run

Actual unmodified chain-command/launch is invoked in fresh mapped user, mount,
PID, IPC, UTS and network namespaces. /work/view contains either real proc/dev
directories, one symlink, or both symlinks. Every symlink targets an owned empty
canary directory under the disposable guest's /work. A trusted diagnostic replaces
entry-helper solely to inspect mountinfo and exit before pivot/image execution.
No filesystem image is mounted; this is the launcher's path-resolution boundary.

Observed: directory control mounts inside /work/view. proc/dev symlink cases
redirect the respective proc/tmpfs mounts to /work/canary-proc or canary-dev.
All four invocations exit; outside mountinfo and sentinel bytes remain unchanged.
Evidence: reproduced/runtime-path. The negative behavior is confined to the child
mount namespace; this does NOT demonstrate supervisor modification or breakout.

Implication: the small policy fixture's pre-created runtime directories cannot
qualify image-supplied runtime paths. Before runtime setup, validate directories
relative to a pinned image root, reject symlink/non-directory targets (including
intermediate components for nested runtime paths), and maintain a no-writer
precondition or use descriptor-bound mounts. A final path-contract implementation
and its actual-FUSE/race controls remain needed. No production/runtime adoption
or acceptance of arbitrary image paths is implied by this diagnostic.
