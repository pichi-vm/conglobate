# M4 partial-setup, containment and deadline review

Baseline 7f0ec9d. Partial self-review; not a clean whole-milestone pass.

- Preparation owns namespace creator failures separately from mounted assigned
  storage. Started helpers retain process handles; failed domain publication
  retains the created domain for inspection. Mounted storage and namespace
  owners remain retained until cancellation, exclusive reaping and child drain.
- Domain publication uses no-replacement staging in a protected directory.
  Rollback distinguishes absent, matching, malformed and mismatched records;
  unexpected records are not deleted. Domain removal and record removal are
  separate transitions. The record validates parent/device/inode identity and
  requires the target absent before removal. A post-unlink sync failure may
  remain unrecoverable and require VM disposal; no cleanup success is inferred.
- ContainedHelper launch separates pre-spawn, identity exchange and join errors.
  Joining capability is consumed before cancellation. Failed acknowledgment
  does not imply image execution never started. Domain cancellation writes
  cgroup.kill and observes populated=0 through one retained event descriptor.
  It is not a reaping receipt: direct helper and adopted descendants are reaped
  separately before record deletion and storage release.
- ChildReaper is process/thread-bound, installed once before children, and uses
  __WALL with exclusive wait ownership. It waits through pidfd observation, not
  timing sleeps. Trusted creator processes outside the command domain are
  individually reaped before process-wide drain.
- SetupBudget begins before preparation. Helpers receive a conservative absolute
  monotonic cutoff rather than a renewed relative timeout. Execution timeout
  starts at admission; the smaller setup/execution cutoff bounds its handshake.
  Teardown and preparation rollback retain their fixed cutoff through every
  consuming transition, including completion checks. Kernel blocking operations
  may outlast a deadline, but late completion is not accepted. VM disposal is
  still external and mandatory when cleanup cannot finish.
- No new production defect established in this subset. Immutable plans and
  meaningful owner methods remain; phase enums carry resources rather than
  independent state flags. Retained/ManuallyDrop storage deliberately avoids
  claiming destructor cleanup proves quiescence.

## Evidence reconciliation

Compared all 130 production Rust files against thirteen preserved VM source
snapshots; exact results and embedded assessments are in
evidence-source-comparison.json. The newest confinement/cache snapshots match
the current production tree. Older failure runs differ only in receiver early
diagnostic emission and the execution retry/diagnostic/pipeline correction.
Their domain/preparation/teardown code is unchanged, but the runs are not
relabeled as fresh whole-program qualification.

Added busy-execution-v1 to exercise the corrected high-level retry API in a VM:
trusted helper reports successful completion, an ordinary directory handle
causes EBUSY, Step::execute returns failure, and closing the holder allows
retry_cleanup to recover only rejected storage while retaining the original
error. Full fixture resource cleanup and independent poweroff pass. This is
not an actual image-command/labeling test. Strict fixture Clippy passes.

Remaining before the M4 three-pass sequence: finish protocol/process/anchor
review and consolidate whole-M4 requirements/style/evidence coverage, then
perform three genuinely complete reviews on frozen scope. M5–M8, current CI,
Arm and confidential-platform qualification remain separate open requirements.
No legacy evidence or uncommitted user work removed.
