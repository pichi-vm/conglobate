Concurrent command failure with a surviving complete build
========================================================
Reproduce from repository root:
  python3 validation/sandbox/guest.py chain-concurrent-failure \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-concurrent-failure --run

Uses the concurrent fixture's actual Chain/Snapshot/FUSE/Record pipeline and
byte-identical base copies. Both workers reach trusted launch gates. Mapping
tables require two independent file-backed stacks sharing one zero origin.
The trusted launcher puts a fixture-only /fail marker in A's writable view.
Both images run the same bounded command, writing derived files/metadata and
starting the existing short-lived heartbeat descendant. A's command then exits
7. The worker propagates that failure and unmounts/reaps FUSE. The helper must
exit 101 (its existing assert), with no verified image or published directory.

B stays mounted, alive and unpublished throughout A's failure. Only after A
exits is B released. B must execute, seal, restage, kernel-verify and publish.
Both original base copies remain byte-identical. Final checks require no DM
mappings or attached loop devices. All activity is in a disposable guest with
no host shares/disks/NIC; gates and whole-guest lifetime are bounded.

The harness splits explicitly delimited A/B logs. A must reach the restricted
command marker and report status 7, with no sealing/restaging/publication marker.
B must independently complete every specified success marker exactly once.
Thus one surviving success cannot disguise publication by the failed build.
Sources, tool/kernel inputs, VM command and log are retained in
validation/sandbox/reproduced/chain-concurrent-failure.

Scope: ordinary command failure and unwinding with a live concurrent consumer.
Not SIGKILL, supervisor/guardian simultaneous loss, interrupted COW recovery,
shared full block-backed chain, cross-build identity separation or production
executor certification. Same mapped UID range is a fixture limitation retained
from the successful concurrent test, not an endorsed security arrangement.
