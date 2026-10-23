# M4-T1 follow-up: bind cleanup cutoff to observation

Baseline 15195b2. Targeted review finding; full M4 clean streak remains 0/3.

## Finding

RunningCommand::observe returned CommandResult<RunningCommand>. Teardown::new
started the fixed budget only when the caller later invoked teardown. The
split API permitted retaining a successful observation for an arbitrary period
before starting a fresh cleanup budget. It also exposed the running owner again
through into_parts, allowing another observation attempt. The production
Step::execute caller immediately chained observe and teardown, so this is an
ownership/API contract defect, not evidence of a delayed production CLI build
or acceptance of unsafe output.

The original M4-T1 correction explicitly required the teardown cutoff to begin
when completion is observed. A fixed duration measured from an arbitrarily
later teardown call does not meet that requirement.

## Correction and source review

Observation consumes RunningCommand into ObservedCommand, which holds the
original owner and one immutable cleanup cutoff. It exposes only the cutoff;
there is no public construction, re-observation, raw command extraction or
caller-supplied replacement deadline. CommandResult retains the primary outcome.
Both completion and observation-error cleanup paths carry the stored cutoff
through crate-private transitions. Direct cancellation before observation still
starts its budget when cancellation begins. Retry retains its existing cutoff.

No mutable acceptance flag, freestanding function or unsafe code was introduced.
The owner is a distinct irreversible lifecycle state, consistent with the
existing RunningCommand/PreparedStep API. Teardown error owners continue to
retain phase and storage; this change does not authorize recovery as accepted
output. Finalization retains the same teardown deadline.

## Evidence

- Rootless suite: 204 tests plus 52 doctests pass. New compile-fail test rejects
  re-observation of an ObservedCommand. Root and fixture strict Clippy pass.
- delayed-teardown-v1: trusted helper reports success and parks before cleanup.
  Fixture holds the observed owner until its cutoff expires. Teardown and retry
  fail within one second, preserving success, volume and recovery record.
- observation-held-cleanup-v1: success observed before gated cleanup; cleanup
  released after the execution deadline but within the cleanup cutoff. Normal
  teardown and storage finalization pass.
- observation-blocked-descendant-v1: see the separately retained assessment;
  required rerun of the actual production init/receiver with D-state descendant.

These regressions cover cutoff retention and both terminal and normal cleanup
paths. They are not full milestone passes, target boots, confidential-platform
qualification or proof of successful disposal of a kernel-blocked task.
All large VM artifacts remain external and not durably archived.
