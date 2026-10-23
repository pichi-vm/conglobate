# COPY/cache review disposition reconciliation

Reviewed current source at baseline 5745b91 against six historically accepted
findings. Four were already implemented and needed evidence-backed disposition,
not repeated implementation. Two still applied and are fixed here.

- ownership-001: move the wildcard-walk candidate after its entries.get borrow
  ends. The candidate has no later use; selected names and resolved paths retain
  their existing ownership and ordering.
- ownership-007: preallocate receipts for all layers and, only when present, the
  config. Every successful loop iteration still contributes exactly one receipt;
  no validation or failure branch was removed. The historical description's
  unconditional plus-one is adjusted for current optional-config handling.
- performance-005 and performance-006: eb767f3 already introduced ordered subtree
  ranges. Current descendants hoists the directory check; wildcard traversal
  retains the direct-child filter. Path-component comparisons exclude dir-x and
  dir2 from dir, preserve nested symlink entries, and handle root/file selections.
  Existing tests exercise these distinctions. No benchmark claim is made.
- performance-001: b17e4a6 already combines initial chain and identity scans.
  Only matching scute digest/size receipts are reused. Config and non-scute
  components still produce authenticated receipts, and public independent
  verification operations remain usable. VerifiedScute::prepare separately opens
  its input, checks the authenticated root, and retains that inode for dm-verity;
  compressed/materialized input is also finished/checked. The initial receipt
  does not authorize a later read of mutable cache bytes.
- simplification-006: eb767f3 removed the unused Step convenience method from
  step/bootstrap.rs. The similarly named Supervision transport operation is
  still used by ExecutionStorage and must remain. Archived validation sources
  legitimately contain the historical wrapper and were not edited.

Validation: 128 library tests, including combined-verification receipt/tampering
and COPY selection tests; 13 cache integration tests; strict all-target Clippy.
All passed. The source hash manifest identifies the inspected files. No new
tests duplicating these small allocation changes were introduced.

This is a focused disposition review, not a full milestone adherence pass or
fresh privileged storage/VM qualification. M4 remains 0/3.
