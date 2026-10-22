Prompt cancellation in the live-build admission topology
======================================================
Reproduce:
 python3 validation/sandbox/guest.py chain-build-stop \
  --kernel-version 7.2.10-200.fc44.x86_64 \
  --worker-build /tmp/conglobate-finalize-workers --out /tmp/NEW-build-stop --run

Actual committed/locked Chain/Snapshot/FUSE build, with the same command/worker
rendezvous and safe-Rust ownership-init as build-admission. Coordinator begins
at Building. While command is active, reserve/commit reject premature publication.
The trusted coordinator pins the dedicated launcher's exact namespace-init child,
checks NSpid and nonterminal pidfd, and requires a live FUSE connection. It accepts
cancellation before sending SIGKILL only to the dedicated launcher's pinned pidfd.
No command-completion rendezvous is released. Namespace init dies through the
existing parent-death ownership link; the coordinator requires its pinned pidfd
terminal within five seconds, independently of the launcher reap.

Passing result: outer namespace termination, no FUSE connections, no DM targets,
no loop attachments, unchanged original base bytes, absent internal/public output,
and prepare/reserve/commit remain Cancelled. The runner additionally rejects any
sealing or completed-artifact publication markers. GUEST_EXIT 0 appears once.
Storage teardown is checked within a separate five-second window. This is a
bounded result, not a latency guarantee under arbitrary kernel or system load.
No passing-path FUSE abort, cgroup kill, namespace-init signal or explicit device
removal is used. The arbiter is reaped separately after assertions via its pidfd.

This extends accepted-cancellation/no-publication with prompt termination in the
same topology. It does not validate coordinator/arbiter death, blocked or cyclic
worker states in this exact composition, simultaneous-owner memory pressure,
authoritative recovery after every lifetime-handle holder dies, or production
integration. Existing stopped-worker/lifetime tests remain separate evidence.
The coordinator process and trusted arbiter still have distinct lifetimes;
source inspection shows ordinary Popen rather than a validated parent-death link
between them. Do not infer owner-death cleanup from this explicit cancellation.

First packaging attempt failed gzip with host Disk quota exceeded before QEMU.
preparation-error.txt records that non-runtime failure. storage-cleanup.json lists
only generated roots/boot files removed after comparing their logs/input catalogs
against retained evidence. A fresh output directory was used for the passing run.
