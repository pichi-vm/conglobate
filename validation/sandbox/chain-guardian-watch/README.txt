Candidate supervisor watch of guardian lifetime
================================================

Reproduce:
  python3 validation/sandbox/guest.py chain-guardian-watch \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-GUARDIAN-WATCH --run

This extends the guardian-death negative control with a trusted supervisor-side
monitor outside the build cgroup. The guardian opens a pidfd referring to itself
and passes it only to that monitor. The monitor launches the actual chain helper
with descriptors closed, holds it stopped until cgroup assignment, then resumes
it. It pins the active fixture's FUSE abort handle before declaring itself armed.
Only one FUSE connection exists in this disposable fixture; this selection rule
is not a proposed multi-build connection-ownership implementation.

The outer test observer requires two growing command heartbeats and the armed
marker before killing the owned guardian. The monitor observes pidfd readiness,
kills the build cgroup, aborts FUSE before waiting, and reaps its helper and
adopted descendants. It checks empty/removable cgroup, absent FUSE connections
and no verified/published output, then records completion. The observer does
not perform cleanup on the passing path. Its fallback is allowed only when the
test is already failing and cannot emit the required passing result.

Observed result: guardian death is detected, monitor cleanup completes, the
observer confirms it did not use fallback cleanup, and the outer init confirms
unchanged base SHA256 and no remaining DM/loop/FUSE resources. The host verifier
rejects sealing/reconstruction/publication-success markers. Evidence:
reproduced/chain-guardian-watch. The earlier negative control remains retained
and demonstrates what happens without this watch.

The monitor is a prototype for responsibility within the trusted supervisor,
not a newly mandated daemon, third-party runtime or production implementation.
Python/ctypes subreaper instrumentation is confined to tests; safe-Rust rules
for the real binary are unchanged. The guardian pidfd is not passed to the
builder/commands. All resources are disposable guest fixture resources.

Scope: guardian loss occurs after monitor registration, active filesystem
registration and two live writers. Readiness and cleanup have explicit bounds;
the guest retains its independent150-second timeout. The driver never requests
normal completion. Therefore this does not qualify guardian loss during startup,
normal completion or final publication, nor simultaneous monitor/guardian death,
resource-starved cleanup, hidden namespace holders or multiple concurrent builds.
The selected protocol still needs those cases and an explicit publication
eligibility rule. A pidfd liveness check immediately before publication alone
would not prove the absence of a subsequent guardian-death race.

Startup boundary follow-up
--------------------------
The fixture now accepts --guardian-watch-stage with before-launch,
before-group-assignment, before-resume or active (the default). Use a new output
directory for each. All four pass in the current source revision; retained
evidence is under reproduced/guardian-watch-startup with one subdirectory per
stage. The successful active rerun guards the earlier behavior after this change.

The initial three stages hold trusted setup after the guardian pidfd is already
registered, before any image command or storage assembly begins. The observer
checks that no helper exists before-launch. At before-group-assignment it checks
the helper is stopped, childless and absent from the cgroup. At before-resume it
checks the helper is stopped, childless and present in the cgroup. Only then is
guardian death injected. No command progress file may appear in these cases.

The monitor retains the helper pidfd independently of cgroup membership. On
guardian loss it cancels that handle if present, then kills the cgroup and
performs the existing abort/reap sequence when a filesystem exists. This closes
the tested created-but-not-yet-assigned child window. Monitor cleanup completes
without the observer fallback; no FUSE/DM/loop resources or output remain and
base bytes are unchanged. The startup cases never invoke the chain builder;
they validate its launch ownership, while the active case exercises real storage.

These checkpoints supersede the earlier blanket statement that all startup
behavior is untested. They do not cover guardian death before watch registration,
failures inside every spawn/registration instruction, concurrent setup, normal
completion/publication races or simultaneous monitor/guardian death. Holding the
helper stopped is a test synchronization mechanism; an implementation must
provide equivalent ownership ordering using its chosen safe library APIs.

Guardian already dead before watch registration
----------------------------------------------
The additional --guardian-watch-stage before-watch-registration stops the
trusted monitor launcher before it execs Python. The guardian has already
supplied its own pidfd, but the monitor has not registered it, created its
cgroup or launched a builder. The observer pins that stopped launcher, verifies
the absent group/helper, kills and reaps the guardian, then resumes the monitor.
Its immediate pidfd poll detects the existing death and refuses builder launch.
Cleanup completes without observer intervention; no helper-started/progress
marker, output or storage resource remains. This passes in
reproduced/guardian-watch-pre-registration. A fresh before-launch live-guardian
control also passes, showing the initial check does not always reject.

The monitor now checks the supplied handle immediately after registration,
before starting a builder. The test covers death between handle handoff and
registration, not a launcher that fails before supplying any handle, every
scheduling race between checks/spawn, simultaneous ownership loss or final
publication. Those distinctions remain part of the lifecycle design audit.
