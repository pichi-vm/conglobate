Cancellation boundaries and launcher death
=========================================

Reproduce:
  python3 validation/sandbox/guest.py lifecycle \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --out /tmp/NEW-LIFECYCLE-GUEST --run

The guest has no host shares/disks/NICs. All signals target fixture processes.
The synthetic command writes one byte every 20ms, at most 500 times, through
a detached double-fork/setsid descendant. The setup barriers last at most 10s.
The trusted driver is a subreaper and retains a pidfd for namespace init before
cancelling. The command uses the existing restricted safe-Rust entry helper.

Negative controls:
- Killing the unshare launcher without --kill-child leaves the writer running.
- With util-linux 2.41.5, --kill-child=KILL plus the fixture's mapped IDs and
  --setuid/--setgid also leaves the writer running. A prior strict run stopped
  at the first setup boundary; /proc showed the surviving bash init and sleep.
- Both negative controls are cleaned up through the retained init pidfd.

Source-grounded explanation of the second result:
The inspected v2.41.5 unshare source sets PR_SET_PDEATHSIG before its later
setgid/setuid calls. Linux documents that changing effective/filesystem UID/GID
clears this setting. This ordering explains the mapped-ID result; it is not a
claim that --kill-child never works. Parent-death signaling is not an adequate
sole cancellation contract for untrusted command code.
https://github.com/util-linux/util-linux/blob/v2.41.5/sys-utils/unshare.c
https://man7.org/linux/man-pages/man2/PR_SET_PDEATHSIG.2const.html

Passing supervised cancellation:
- After user namespace/ID setup.
- After making and binding the private root mount.
- After mounting private proc.
- After private runtime-device mounts, before entry-helper execution.
- During the restricted command, after the detached writer has started.

For each, kill the launcher, explicitly signal the tracked namespace init using
pidfd_send_signal, reap all adopted children, verify pidfd readiness, verify an
unchanging heartbeat and verify the supervisor mount table is unchanged.
The active supervisor survives and can launch the next case. The tests do not
infer completion merely from successful kill() or launcher exit.

Evidence:
  ../reproduced/lifecycle/{inputs.json,command.json,results.txt}
  ../reproduced/lifecycle-kill-child-failure/{inputs.json,command.json,results.txt}

Limits:
- This is not cancellation at every storage/network/export setup boundary.
- PID acquisition here occurs after a known fixture barrier. Atomic/race-safe
  acquisition and cancellation before that point are unresolved integration
  gates. The future launcher must obtain authority before exposing commands.
- Supervisor death itself, recovery by an independent monitor/cgroup owner,
  FUSE hangs, concurrent builds and no-publication on failure have separate gates.
  ../guardian/README.txt now supplies bounded evidence for an independent
  cgroup owner's cleanup after supervisor death; full build teardown stays open.
- Commands cannot create nested namespaces under the candidate syscall policy;
  this fixture tests ordinary detached process trees, not nested namespace trees.
- Python ctypes prctl is isolated test instrumentation. It is not a production
  unsafe-code exception. Rustix exposes safe pidfd APIs in the locally inspected
  1.1.4 source; this fixture has not yet qualified their production integration.
- The test does not adopt unshare as the production runtime.
