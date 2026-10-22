Blocked command I/O cancellation through authenticated snapshot setup
====================================================================
Reproduce:
  python3 validation/sandbox/guest.py chain-blocked-cancel \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-BLOCKED-CANCEL-GUEST --run

Uses the finalization-patched worker from ../finalize, actual committed/locked
Inputs resolution, authenticated Chain/Snapshot setup and the restricted entry
helper. Guest has no host shares, disks or NICs; its150-second timeout bounds
fixture failures. Source and input catalogs: ../reproduced/chain-blocked-cancel.

The active-cancel gated pidfd handoff and two bounded writer commands are reused.
Once both counters grow, the trusted mapped driver stops the filesystem worker
through its pidfd. Initial-userns controller requires exactly one FUSE connection,
retains its abort FD and waits for waiting>0 before authorizing cancellation.
The mapped driver SIGKILLs namespace init through its pinned pidfd and confirms
signalling. Controller aborts the connection BEFORE client reaping, avoiding the
dependency reproduced by the earlier pressure tests. The driver then reaps the
launcher; mapped worker wrapper kills/reaps the stopped daemon and unmounts.

Worker exit125 triggers the chain probe's expected assertion/panic exit101 before
Snapshot::finish. Controller requires total cancellation/cleanup under10 seconds,
unchanged base SHA256, no published/reconstructed image and no remaining dm,
loop or FUSE connection. Host verifier rejects layer-sealed/restaged/publication
markers. All pass. No output is derived from the abruptly stopped filesystem.

This integrates blocked-I/O abort with real storage setup and the sealing gate;
it does not qualify a cyclic callback deadlock, arbitrary kernel-stuck tasks,
supervisor/guardian death, concurrent build identity, production signal handling,
or cancellation during COPY/export/sealing/rename. Error paths before the
expected handshakes remain bounded by VM disposal, not a demonstrated general
cleanup implementation. The worker here is the finalization prototype, not the
newer ACL protocol pair. Production executor code is unchanged.
