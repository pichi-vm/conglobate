# Current lifecycle historical-finding reconciliation

Source review at b9830a9, no production changes. This is a focused reconciliation,
not one of three complete M4 adherence passes. Original evidence/status/action
fields remain verbatim; current dispositions are recorded separately.

## concurrency-001

revalidated rejection: ReceiverStartup::failure only joins DiagnosticRelay after successful wait; a failed wait retains a potentially live writer. Unconditional joining could violate bounded cleanup. Retain rejection of that proposed fix, without promising delivery of every startup diagnostic.

## concurrency-002

revalidated rejection: ChildReaper checks the original deadline after a collected child; this does not yet establish ECHILD. Teardown/Finalization independently check the same deadline before successful receipts. Do not treat an unobserved last-child condition as on-time completion.

## simplification-001

revalidated implementation: One generic Finalization<R: PreparationStorage> owns Mounted/Retry/Complete transitions and retained diagnostics/deadline. Bootstrap duplicate is absent; StorageLayout LABEL is bootstrap for paired volumes and step for single volumes, addressing the historical Display follow-up.

## simplification-007

revalidated rejection: Step<Ready>::with_volume remains the compile-fail ownership seam in src/step.rs; its consuming transfer prevents subsequent volume use. Keep the deliberate typestate test surface; do not infer unused API from production caller search alone.

## simplification-019

revalidated rejection: PreparedScute::data_size/hash_size have maintained cache-storage fixture callers, not only historical validation users. They expose numeric diagnostics without raw file/control authority. Retain these methods.

## tests-009

revalidated rejection of stale coverage claim: Maintained contained VM fixture installs ChildReaper before children, synchronizes via pipe/datagram, cancels/reaps/drains adopted descendants, checks collected >= 1 and empty child/record/domain state. Fresh native amd64 run passes launch/join failure and abrupt helper death paths. No need to force process-global reaper setup into a threaded unit harness; isolated subprocess tests are possible in principle. Full architecture/platform coverage is not claimed.


Fresh native amd64 contained VM passes all required markers, exit zero and VM
shutdown. It includes positive descendant readiness before cancellation/abort,
actual collected descendants, empty /proc/thread-self/children and removed owned
record/domain directories. This is stronger than merely seeing process exit but
not a census of every kernel object or a general crash-recovery guarantee.

225 rootless tests and 53 doctests from m4-checkpoint-cedb69d apply to unchanged
production source; they are not rerun or described as new evidence. This run does
not directly exercise the generic storage finalizer's fault branches. The source
review checks its consuming states, original cutoff, validation and retained
owners; previously recorded storage failure evidence keeps its original scope.

Source comparison covers the actual archive. Raw status, logs, source and runtime
inputs retained; large assets locally hashed without remote durability claim.
Remaining original findings and complete milestone gates remain outstanding.
