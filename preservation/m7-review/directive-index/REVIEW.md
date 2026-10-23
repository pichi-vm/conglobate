# Stage-relative carapace error position

Carapace iterates with its stage-relative index and passes it to consuming
StageFailure::reject_at. Whole-stage bootstrap/firmware rejection retains its
existing index. All six failure variants retain the same cleanup transitions
and owners; no nested or contradictory directive indices are constructed.

150 library tests and strict all-target Clippy passed. Maintained
 tests/distro/directive-index/ runs two carapace commands: first succeeds, second
exits 7. Native amd64 KVM run directive-index-v1 returned build failure as
expected; the verifier passed the second-position, first-command, cleanup and
no-export checks. All 170 archived source files compared match the worktree.
The verifier's initial premature invocation while the VM was running is retained
as premature-assessment.json; it is not a terminal VM failure. Final result is
vm/directive-index-assessment.json. Cleanup assertion is a diagnostic check, not
independent enumeration of every kernel resource. Large VM assets remain local,
with hashes and locations recorded; no durable remote archive is claimed.

This closes the separate position defect from pipeline-error-audit/REVIEW.md.
No complete milestone adherence pass or broad qualification is claimed.
