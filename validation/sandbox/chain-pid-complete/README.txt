Normal full build inside the outer PID lifetime namespace
========================================================
Reproduce:
  python3 validation/sandbox/guest.py chain-pid-complete \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-pid-complete --run

Disposable guest, no host shares/disks/NIC. A small trusted prototype runs as
PID 1 with a private proc mount and starts the actual Chain/Snapshot/FUSE/
restricted-command/Record helper. The ordinary command produces derived bytes
and ownership changes, with the existing detached-writer teardown check.

Normal completion requires helper success, published inventory, and no remaining
children/waitable descendants BEFORE PID init exits. Namespace exit cannot hide
live descendant leaks in that check. Existing helper assertions cover actual
sealing, authenticated restaging, kernel filesystem/content/ownership validation,
unchanged base and inventory publication. Outer init then exits normally and the
parent requires no DM mappings or attached loop devices. Helper wait is bounded
to 90 seconds, guest lifetime to the runner's 150 seconds.

This is a basic normal-owner control for the outer PID lifetime boundary, not a
final implementation of the guardian/monitor completion protocol. The same
kernel namespace containment primitive wraps the failure fixtures, but here PID
init directly waits for the builder; normal monitor handoff/attestation and
publication cancellation eligibility are still unresolved. No production code
changed. The trusted prototype uses Python/util-linux only as validation tools.

Result: full pipeline passes. Initial init assumed no adopted child could remain
and failed after publication. Follow-up records the child as State Z (zombie),
reaps it, and still requires no live/remaining descendants before normal exit.
It never signals a live child to make this check pass. Evidence:
reproduced/chain-pid-complete-unreaped (initial failure) and
reproduced/chain-pid-complete (successful init reaping). Zombie reaping is a
required trusted-init responsibility, not evidence of continuing writes.
