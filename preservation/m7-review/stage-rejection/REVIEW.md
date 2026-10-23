# Shared terminal stage recovery

Implements accepted simplification-004 after 0a12592. A pipeline-private
DiscardStorage trait on unmounted storage supplies single-volume and bootstrap
pair disposal. StageFailure::reject uses DirectiveStorage's associated unmounted
type to share the six-arm recovery walk. All pipeline callers now consume their
failure through that method; firmware no longer calls a carapace coordinator's
rejection helper.

Reviewed invariants:
- Save primary diagnostic before cleanup consumes the failure.
- Before/Preparation recover only unmounted storage; Execution/NetworkExecution
  use rejection-only retry and into_rejected_storage.
- NetworkSetup cleans its separately owned worker before command teardown.
- COPY preserves its rejection and discards recovered storage only.
- Bootstrap attempts both tool and output disposal and preserves both errors.
- The shared method returns BuildError in every branch, including cleanup success.
  There is no successful stage receipt, sealing or continuation path.
- The discard trait remains private; its bound is on the private reject method,
  not a public inherent impl. No unsafe code or freestanding function introduced.

Validation: 126 library tests passed, including rejection-only cleanup retry
regressions. Strict all-target Clippy initially found a private-bound visibility
issue, then passed after moving the bound onto the private method. Both logs are
preserved. Formatting and git diff --check passed. No new mirror tests or VM run;
current CLI failure-path qualification remains outstanding. Typed error rendering
at the pipeline boundary is unchanged and remains separately tracked.
