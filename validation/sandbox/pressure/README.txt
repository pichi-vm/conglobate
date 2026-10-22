Bounded FUSE request pressure and noncooperative-worker cleanup
=============================================================

Run:
  python3 validation/sandbox/guest.py pressure \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --out /tmp/NEW-PRESSURE --run

The disposable guest creates a fresh 64MiB ext4 image for each of two cases.
A mapped user/mount/network namespace owns fuse2fs. Preparation creates 32
distinct directories, then stops the daemon and starts exactly 32 touch
processes, one per directory. Distinct parents avoid directory-lock
serialization. The trusted initial namespace requires fusectl waiting >=32
before continuing and retains that connection's abort descriptor.

abort:
  Abort the connection with clients still waiting. Every client must exit1
  with an abort/disconnected diagnostic. Reap all clients, kill/reap the stopped
  daemon, unmount, close the control descriptor, and verify connection removal.

clients-first:
  Signal all clients with SIGKILL while the worker is stopped, then abort the
  connection before waiting for every client (each status137). A subsequent stat must fail with the expected
  connection error, not hang until its ten-second timeout. Kill/reap the
  stopped daemon, unmount and verify connection removal.

Both cases assert the recorded process IDs are absent after wait and the mount
is gone. Cleanup after abort must take under ten guest seconds; the host runner
also bounds the entire disposable guest to 150 seconds. No host mounts, host
network or host block devices are exposed. Test operations and process counts
are fixed; this is fault-injection test code, not an exploit or load generator.

Evidence: ../reproduced/pressure/{inputs.json,command.json,results.txt}.
The input catalog includes binaries, libraries, scripts and the guest kernel.

Primary reference (checked 2026-10-20):
  https://docs.kernel.org/filesystems/fuse/fuse.html
The documented waiting counter includes requests queued for or handled by the
daemon; abort fails outstanding and future requests. Fatal signals can remove
requests not yet delivered to userspace. This test stops the worker before
submitting requests, so it does not test interrupting an active callback.

Limits:
- 32 synchronous operations, not background-request-limit saturation, memory
  exhaustion, throughput qualification or proof for arbitrary request counts.
- Stopped worker, not a cyclic callback/page-fault dependency.
- These are filesystem transport primitives; clients do not use the restricted
  recipe launcher in this fixture.
- No sealing/publication is attempted after abort. Integration with failed-build
  state, cancellation and inventory publication remains a separate gate.
- No claim that an aborted filesystem is suitable to publish or even fsck-clean.

Qualification history / failed cleanup ordering:
The initial implementation waited for every signaled client before allowing
connection abort. It passed some runs, but repeated testing stalled in wait
until the supervisor timed out. Retained diagnostic negative control:
  ../reproduced/pressure-reap-before-abort-failure/
It includes the earlier init/worker sources as well as inputs and output.
An earlier, less diagnostic failure is also retained separately.

Do not rely on client termination/reaping to precede abort. The fixed handshake
signals all clients, notifies the supervisor, then waits while the supervisor
aborts independently. This avoids the circular dependency in the test itself.
The documented queued-vs-delivered request distinction is consistent with the
observation; this probe does not log kernel request transitions to establish
which particular request blocked. It demonstrates that the original ordering
was unreliable, not a kernel defect. Current cleanup checks retain exact wait
statuses and bounded /proc disappearance checks.

The corrected guest passed its initial run and three reruns of the exact same
packaged inputs; repeat logs are retained beside results.txt.
