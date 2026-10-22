Active command cancellation through authenticated chain setup
=============================================================
Reproduce:
  python3 validation/sandbox/guest.py chain-active-cancel \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-ACTIVE-CANCEL-GUEST --run

Build the worker as described in ../finalize/README.txt. This uses that patched
finalization worker, not the later ACL protocol/library pair. The disposable
guest is offline with no host shares/disks and the standard150-second limit.

Real Inputs::resolve verifies a committed Git context/lock; Chain/Snapshot APIs
verify and assemble the base and create the writable snapshot. The mapped
worker mounts FUSE. Trusted launcher reports its ancestor-proc PID before pivot
and waits at a gate. The trusted driver pins it with pidfd_open before releasing
that gate. Command code cannot access that directory after pivot.

The restricted command asserts namespace/capability boundary properties and
runs two bounded60-second writers (namespace-init shell plus child). The driver
requires both counters to grow, sends SIGKILL through the pinned namespace-init
pidfd, reaps the outer launcher within10 seconds and requires both counters to
stop. Worker trap unmounts FUSE and waits for its daemon. Exit125 propagates to
the chain helper's existing worker-status assertion, which intentionally panics
with exit101 before Snapshot::finish. That expected probe panic is not a
production error-handling design.

The guest requires no published output or reconstructed image, unchanged base
SHA256, no dm/loop devices and no FUSE connections. Host verifier additionally
rejects any layer-sealed/restaged/inventory-publication marker. All pass. Input
and source snapshots are retained in ../reproduced/chain-active-cancel.

Scope: bounded active-command cancellation connected to real storage setup and
the pre-sealing failure gate. It does not test the production supervisor's
SIGINT/cancellation handler, supervisor death, stopped-worker/blocked-I/O abort,
cancellation during COPY/export/sealing/rename, or all setup/teardown boundaries.
The guardian and queued-request fixtures cover different pieces; combining
those pieces remains required. No production executor behavior changed.
