# Borrow domain-parent paths through cleanup

Accepted finding ownership-010 was still present at 66169f1. Failure owners
already carry the runtime/recovery lifetime; cloning their domain parent does
not make those owners independent of runtime storage.

CommandSandbox::domain_parent now exposes the existing deployment lifetime.
StoragePreparationFailure, NetworkStageFailure and Teardown borrow that path.
Preparation creation/launch, command execution, observed completion and teardown
entry points propagate the same lifetime. This removes five path clones without
changing phase transitions, original deadlines, failure diagnostics or ownership
of namespaces, processes, volumes and recovery records.

Review checked all seven changed modules and the maintained fixture callers.
Retried failures keep the same borrow; successful finalization can release it.
The compile-fail example is also compiled independently: E0515 specifically
rejects returning an owner that references a local parent path, rather than
failing because of an unrelated missing import or type error.

Validation: 128 library tests, 54 doctests and strict lifecycle Clippy passed.
Logs include checks before and after extending the change to teardown. This is
a lifetime/API simplification, not a fresh VM/platform qualification or complete
milestone adherence pass. Historical source and all existing VM evidence remain
preserved. M4 remains 0/3.
