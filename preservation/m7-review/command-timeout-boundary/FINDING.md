# M4-T1: command completion is reported after cleanup

Baseline 9fb04a0. Confirmed by source tracing, not a new timed VM reproduction.
This interrupts the attempted integrated M4 review; clean-pass streak stays 0/3.
Prior partial reviews did not identify this mismatch and must not be treated as
proof that command/teardown timing conforms to the design.

## Requirement and actual path

SANDBOX-DESIGN 7.6 says execution timeout ends at command completion; remaining
descendant cleanup belongs to teardown. Sections 10.2–10.4 require a separate
fixed cleanup deadline and failure rejection without unsafe resource release.

PreparedStep::start begins the execution cutoff at admission. RunningCommand::
observe waits for Supervision::completion_until under that cutoff. However:

1. NamespaceInit waits for the shell and exits when the shell ends. PID-namespace
   exit entails kernel termination of remaining namespace members.
2. ReceiverStartup::finish waits for the namespace helper and drains stdout/stderr.
3. Role::Receiver then releases resolver scaffolding, closes the mounted view,
   and restores its mount namespace.
4. Only afterward it calls report_completion_until. The supervisor starts its
   teardown budget only after observing that message or an execution error.

Thus a command that exits before its recipe deadline can be classified as an
execution timeout because descendant termination, diagnostic drainage or local
mount cleanup took the remaining time. This is a false rejection/timer-contract
violation, not demonstrated acceptance of unsafe output. Source comments in
admission.rs already describe completion as preceding cleanup, contrary to the
actual receiver caller. Waiting solely for PID-1 exit also cannot separate a
shell exit from a descendant stuck during namespace exit.

## Correction constraints

Moving the existing report call earlier is insufficient. Current Step teardown
immediately cancels the entire domain, including the receiver; doing that before
resolver restoration could skip mandatory cleanup. Existing helper completion
also supplies the only evidence that local resolver/view cleanup succeeded.
Keep rejection-only retry and explicit storage finalization intact.

A replacement needs two distinct observations with separate authority:

- A restricted, status-only report from resident namespace init when it reaps
  the recipe shell. It must occur before PID-namespace exit and cannot carry
  outside-root, backing or supervisor-control authority to the shell. Keep
  CLOEXEC, the PID-1 non-dumpable setting and actual child FD audits. Audit any
  necessary narrowly scoped init descriptor allowance; do not weaken the
  current blanket child descriptor boundary.
- Receiver local-cleanup completion, reported only after namespace child exit,
  diagnostic drainage, resolver restoration and mount-view closure. This runs
  under the supervisor's one fixed teardown cutoff, not a fresh recipe timeout.

The supervisor must start its teardown cutoff when it observes shell status,
retain the primary outcome, and require receiver cleanup before accepting
normal cancellation/drain/finalization. Error/timeout paths still cancel and
retain ownership if cleanup cannot complete. Receiver death or cleanup failure
must never turn shell success into build success. Protocol types should consume
states (running -> command observed -> cleanup observed), rather than expose
mutable acceptance flags. Exact transport implementation remains to be designed
against the current gate/descriptor APIs before editing production code.

## Required regressions

1. Deterministic trusted fixture holds local cleanup behind an explicit pipe or
   socket gate after shell completion. Observe command status before releasing
   cleanup; no synchronization sleeps. Verify cleanup remains necessary.
2. Real command exits with a still-live descendant: separate primary shell exit
   from descendant quiescence and retain teardown bounds. Blocked-I/O fixture
   must not be misreported as an execution timeout after shell exit.
3. Cleanup error after successful shell exit rejects output and retains original
   diagnostics through retry; helper death before cleanup acknowledgment rejects.
4. Actual recipe confirms the status channel is absent from image-command FDs,
   inaccessible through PID 1, and cannot be spoofed through diagnostic output.
5. Existing timeout, nonzero, resolver cleanup, writeback, retained ownership and
   no-publication regressions remain valid under the revised protocol.

No production change, new test pass or qualification claim in this finding.
Next action is the protocol/owner correction and these targeted regressions;
do not continue unrelated cleanup to avoid this acceptance blocker.
