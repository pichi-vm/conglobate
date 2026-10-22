Guardian OOM with an independent supervisor-side watch
=====================================================
Reproduce:
  python3 validation/sandbox/guest.py chain-guardian-oom \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-guardian-oom --run

Disposable guest, no host disks/shares/NIC. This is the active-build boundary of
the candidate guardian-watch arrangement, not its startup matrix. Observer starts
the guardian stopped before exec, places it in a separate memory cgroup with swap
prohibited, and resumes it. Guardian passes its own pinned pidfd to a trusted
monitor launcher, which moves itself out of that cgroup before loading Python
or launching the build. Monitor/build descendants are therefore not members of
the constrained guardian cgroup. Observer also remains outside it.

After two real command writers progress and the monitor arms its FUSE abort
handle, observer requires guardian-only membership/nonzero accounting, then
writes memory.max=0. Kernel memory.events oom_kill plus pidfd/reap status must
confirm actual guardian OOM; passing path sends no synthetic SIGKILL to guardian.
Monitor must detect guardian loss, kill the owned build subtree, abort FUSE and
reap before recording completion. Observer checks stopped writer progress,
removed build cgroup/FUSE connections, and reaps monitor success. No observer
build cleanup is allowed on the passing path; fallback remains failure-only.

Init requires unchanged base bytes, no verified or published output, no residual
DM/loop/FUSE resources. Harness forbids seal/restage/publication success markers.
Observer removes only its now-empty guardian pressure cgroup after monitoring
completes. Existing bounded monitor/observer waits and whole-guest timeout apply.

Scope: isolated guardian OOM with healthy monitor/kernel and available cleanup
resources. Memory charges can survive process migration; no precise RSS-accounting
claim is made. This does not qualify simultaneous guardian/monitor loss, system-
wide OOM, pre-handoff failure, publication races or final executor integration.
The monitor remains an experimental Python lifecycle prototype, not production.

Result: all required outcomes pass. Evidence: reproduced/chain-guardian-oom.
