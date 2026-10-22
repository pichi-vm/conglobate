Active-build supervisor memory-controller OOM
===========================================
Reproduce:
  python3 validation/sandbox/guest.py chain-supervisor-oom \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-supervisor-oom --run

Disposable guest, no host disks/shares/NIC. Uses the actual authenticated chain
and restricted two-writer command from the supervisor-death fixture. Supervisor
and workers occupy sibling cgroups under the owned build group. A trusted test
launcher moves itself into workers before executing unshare/userns setup, so all
worker/command descendants inherit that placement. The helper's optional
CHAIN_WORKER_LAUNCHER override exists only in validation code, not the production
binary. Guardian stays outside the constrained build groups.

After both command writers demonstrably progress, the guardian pins independent
read-only base consumers and the FUSE abort handle. It checks the supervisor
cgroup contains only the expected PID and has nonzero accounted memory, and that
workers occupy their sibling. It writes supervisor memory.max=0 with swap
prohibited, requires memory.events oom_kill to increase, and observes the pinned
supervisor pidfd/reaped SIGKILL status. The fixture does not directly SIGKILL the
supervisor to manufacture the OOM result.

Then the normal guardian path kills the owned build subtree, aborts FUSE before
waiting, reaps descendants and removes all cgroups. Independent base readers
remain valid until closed; deferred mapping/loop cleanup then completes. No
verified image or publication exists and the base SHA256 remains unchanged.
The harness forbids sealing/restaging/publication markers. Guardian checks have
bounded waits and the whole guest has the runner's 150-second timeout.

Evidence: reproduced/chain-supervisor-oom. This validates supervisor-only cgroup
OOM during active commands with a healthy, unconstrained guardian. Memory charges
need not equal only the supervisor's current RSS: inherited pages and earlier
allocations may stay charged to the originating cgroup. The test makes no precise
accounting/isolation claim beyond the observed membership/events. It does not
qualify global OOM, guardian pressure, simultaneous owner failure or cleanup
without available kernel resources. No production code changed.
