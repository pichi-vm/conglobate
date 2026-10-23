# M4-T1 current coverage reconciliation

Reviewed against 2dc93c5ff2921eafec3fe662bb644f384b98499c. This is a targeted
source/evidence-scope review, not a full milestone adherence pass.

## Implementation traced

- `NamespaceInit::execute` reports the reaped shell's raw status before exiting
  PID 1. Namespace exit and termination of remaining members happen afterward.
- `ReceiverStartup::finish` forwards that status before waiting for the namespace
  helper and diagnostic relay. `Role::Receiver` reports cleanup separately.
- `Teardown::run` waits for cleanup acknowledgment under its fixed deadline.
  Failure retains the running step, and retry uses the same deadline. Shell
  success alone cannot yield finalized storage.

This source ordering corrects the original late-status path. It is not evidence
that every kernel-blocked case has been executed successfully.

## Regression scope

1. Held receiver cleanup: maintained `sandbox-held-cleanup` exercises early
   status with an explicit release socket; the command deadline expires while
   cleanup remains held. This is a trusted helper, not a recipe shell.
2. Live recipe descendant: confinement recipe uses FIFO readiness, confirms a
   background child is alive, and exits. The subsequent step and output checks
   pass in descendant-status-v1. That child is interruptible, not in D state.
3. Cleanup failure: missing-cleanup, cleanup-timeout-v2 and busy-execution
   exercise missing acknowledgment, fixed expiration and rejected recovery.
   Timeout retains the recovery owner until explicit process exit.
4. Descriptor boundary: actual confinement verifier requires the descriptor
   marker and rejects unexpected descriptors. Recipe probes PID-1 process
   interfaces. These are scoped observations, not exhaustive proof against
   every possible status-channel access or spoofing mechanism.
5. Network completion: status-network-nonzero/timeout/resolver-v1 rerun failure
   and successful resolver restoration paths under the new protocol.

## Unclosed blocked-descendant case

`BlockedIo::helper` awaits admission, then writes and synchronizes a file on the
fixture-owned suspended mapping. It never calls `report_completion_until`.
`SandboxFailures` therefore expects `ObservationFailed(TimedOut)`. This is a
valid pre-completion timeout fixture; relabeling its result would be incorrect.
It does not instantiate the production namespace-init reporting path.

The missing integration regression must:

- run the production namespace init and recipe shell;
- coordinate a benign child write to a fixture-owned mapping using explicit
  readiness/release messages, and observe actual D state before shell exit;
- observe the shell result before the recipe deadline even while namespace
  exit remains blocked;
- fail under the original fixed teardown cutoff, retaining storage and primary
  shell outcome, with no publication or deadline extension on retry;
- dispose of the VM independently without claiming successful resource cleanup.

Do not substitute a parked helper or an interruptible FIFO child for this case.
The trusted fixture must arrange mapping suspension externally; image commands
must not acquire device-mapper or supervisor-control access for the test.

M4-T1 remains open for this integration regression and final source/evidence
review. M4 remains 0/3. No new VM execution occurred in this reconciliation.
