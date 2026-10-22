Real worker/command writable-handle containment — 2026-10-21
==========================================================
Run guest.py chain-writer-containment with the standard kernel/worker arguments.
This extends the safe-Rust outer-init/stopped-worker active-build fixture. It
changes no production executor and adds no runtime dependency.

Source topology:
Dedicated launcher -> ownership-init/guardian (outer PID 1) -> monitor -> real
Rust chain supervisor -> mapped-userns worker shell -> fuse2fs and restricted
command launcher -> nested command PID namespace. chain.rs retains snapshot and
loop/origin handles while waiting on worker status. The daemon opens the writable
snapshot and /dev/fuse. Nested commands get a mounted root and no device/control
paths; the ordinary heartbeat commands now retain their own image-file FDs so
this test can inventory actual persistent writable handles.

The observer first requires two live writers, stops the actual FUSE daemon and
observes queued requests, then stops the monitor. A read-only /proc inventory
runs before killing the dedicated launcher. It classifies writable descriptors
using fdinfo flags, readlink targets and per-process mountinfo:
- /dev/dm-* and /dev/loop* (excluding loop-control);
- /dev/fuse;
- regular image handles whose mount filesystem is fuse, fuseblk or fuse.*;
- fusectl handles, classified separately as control authority;
- directly open scratch data paths matching /tmp/conglobate-*/.../data.
It requires at least two writable block FDs, one fuse-device FD and two image
FDs, plus at least one fuse-control handle. Every matched holder must have the actual outer init in its PPid ancestry.
No image getattr/read/write is performed by the audit while the worker is stopped.

The observer then kills only the pinned dedicated launcher. Existing ownership
handoff triggers outer-init death. After its pinned pidfd becomes terminal, the
same guest-wide FD inventory must find no matched writable handles. Existing
checks additionally require empty build cgroup, released FUSE connection, stopped
writer progress, unchanged base, no seal/publication and eventual DM/loop cleanup.
No cgroup.kill, FUSE abort or monitor cleanup is permitted on the passing path.

Scope:
This joins actual worker/supervisor/command FD observations to the namespace
termination barrier, rather than assuming separate prototype checks compose.
It does not give a recovery process a persisted pidfd, solve loss of all trusted
lifetime handles, or implement orphan collection. The /proc scan is a bounded
observation of selected FD classes in this fixed guest; it is not an atomic
inventory of every capability. Transient disappearing unrelated processes are
skipped, while required persistent handles have explicit minimum counts. Queued
SCM_RIGHTS, registered io_uring FDs, mappings and kernel-only references are not
exhaustively enumerated. Existing final device/FUSE checks constrain residual
kernel resources in this run. General reference/UID lifetime remains open.
