Independent cleanup after supervisor death
=========================================

Reproduce:
  python3 validation/sandbox/guest.py guardian \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --out /tmp/NEW-GUARDIAN-GUEST --run

No host shares, disks or NICs are exposed. The guest-only supervisor launches
the existing restricted lifecycle command, including its finite detached writer.
An independent trusted process owns a fresh cgroup outside its own membership.
The supervisor stops itself before any descendant creation; the guardian
obtains its pidfd, assigns it to the group, then resumes it. Descendants inherit
membership. This explicit gate avoids the fixture's fork-before-placement race.

Cases:
- Negative control: supervisor SIGKILL alone leaves the writer running.
- Supervisor death after assignment but before launching commands.
- Supervisor death while namespace setup is paused after mapping.
- Supervisor death while the restricted detached writer is active.

On supervisor pidfd readiness, the guardian writes cgroup.kill, waits for
cgroup.events populated=0, reaps all adopted children, verifies a stable
heartbeat and unchanged guardian mount table, then removes the empty cgroup.
The negative control is cleaned up with the same procedure. All four cases
complete, and the guardian remains alive to execute subsequent cases.

Evidence: ../reproduced/guardian/{inputs.json,command.json,results.txt}.
All packaged executables/libraries and fixture source hashes are retained.

Primary mechanism reference:
https://docs.kernel.org/admin-guide/cgroup-v2.html
The kernel documents inherited membership, that moving a process does not move
already-existing descendants, and that cgroup.kill kills the subtree while
handling concurrent forks/migration. The creation gate is therefore material,
not merely a convenient delay before this test.

Scope and unresolved integration:
- This qualifies a mechanism available to a trusted cleanup owner. It does not
  require the conglobate binary to provision resource limits or select an
  appliance/service-manager design. Environmental resource policy is unchanged.
- The trusted monitor must remain alive and outside the doomed group. Its own
  death, machine failure and persistent recovery are not solved by this test.
- The gate precedes the fixture supervisor's first fork; the complete production
  launch/registration handshake still needs a safe implementation and tests.
- No FUSE worker is present. A complete design must decide group membership and
  ordering for command writers versus trusted worker flush/abort/reap, revoke
  writable backing access, and reject publication. This fixture proves none of
  those later stages.
- No hardware attestation, package build, simultaneous-build group ownership or
  uninterruptible filesystem request is tested here.
- Python ctypes subreaper setup is test instrumentation, not a production
  exception to the safe-Rust requirement.
