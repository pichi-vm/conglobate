# Cancellation and setup cleanup review

Two accepted actions still applied at baseline 33e1d4d.

- simplification-005: Ready and Running called helper.cancel_until; Cancelling
  called helper.retry_until. All three repeated identical owner reconstruction.
  A private Step<Reaping>::after_cancellation now performs that reconstruction.
  The public typestate entry points remain separate. Successful cancellation
  retains namespace, storage and reaper in Reaping; failure retains the same
  owners in Cancelling with the original source error. No cloning, new budget,
  admission path, mutable state flag or unchecked transition was introduced.
- errors-api-009: the historical finding is NetworkSetupFailure::cleanup, not
  similarly shaped storage cleanup. Its single match now returns success for
  Complete before applying an expiry guard to unfinished states. Expiry returns
  the original owner. Remaining resource-specific branches and post-operation
  deadline handling are unchanged. The old unreachable arm was genuinely
  unreachable; this was a clarity improvement, not a demonstrated panic fix.

Reviewed the diff against the three original cancellation bodies and network
cleanup branches. Validation: 128 library tests, 54 doctests, all-target strict
Clippy, and two maintained native amd64 disposable-KVM regressions. The network
case covers allocation contention, partial setup rollback, foreign NAT-table
preservation, allocation release and expired cleanup-owner retention. The
attached-helper exit case covers Ready cancellation and complete mount/domain/
child rollback. Source snapshots match the current implementation.

VM evidence is preserved in sibling cancellation-network-v1 and
cancellation-attached-v1 directories. This does not freshly qualify every Running
and retry branch, target distro boots, other architectures or confidential VMs.
Large VM assets remain locally retained with hashes, not remotely durable.

Focused review only; M4 remains 0/3 complete clean adherence passes.
