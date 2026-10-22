Controlled cyclic FUSE callback dependency
==========================================

Reproduce:
  python3 validation/sandbox/guest.py cyclic-fuse \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --fuse-headers /tmp/conglobate-finalize-workers/fuse-headers \
    --out /tmp/NEW-CYCLIC-FUSE --run

The C program is test-only instrumentation, not production code. It exposes one
read-only file in a disposable guest. Its single-thread read callback attempts
a read through the same FUSE mount, creating a deliberate callback dependency.
No host shares/disks/NIC are attached. The worker is inside mapped user, mount
and network namespaces; its marker/log files are outside its own FUSE mount.

An independent initial-namespace controller waits for callback entry and at
least two outstanding requests in fusectl. It verifies that neither the client
nor the nested operation has completed, then writes to the pinned abort FD.
Observed: external client returns ENOTCONN (107), callback's nested operation
returns ECONNABORTED (103), worker and client exit, unmount succeeds and the
connection disappears. Cleanup completes under the measured ten-second check.
Startup/queue observation is bounded; the host QEMU watchdog is 150 seconds
if an unexpected kernel/fixture stall prevents ordinary reaping. No retry loop
or host mount is used to reproduce the dependency.

Primary mechanism documentation:
https://docs.kernel.org/filesystems/fuse/fuse.html
Sections on control filesystem, abort and deadlock describe the outstanding
request counter and connection abortion. The test exercises a controlled
single-thread dependency, not every deadlock scenario described there.

Limits: no fuse2fs block image, filesystem corruption, signal-based cancellation,
background writeback cycle or integrated publication pipeline. The test daemon
exits successfully after abort; that does NOT authorize publishing a build.
The supervisor's explicit aborted/failed state must dominate worker exit status.
Safe-Rust production requirements are unchanged. The fixture's C syscall code
and native library are confined to disposable-guest test instrumentation.

Evidence: reproduced/cyclic-fuse includes exact fixture sources, compilation
command and header hashes/content, linked-runtime hashes, kernel, guest command
and output. The supplied headers and host libfuse are explicit build inputs.
