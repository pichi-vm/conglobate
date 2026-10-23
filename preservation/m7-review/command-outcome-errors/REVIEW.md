# Typed command rejection and retained recovery history

Baseline: 870ac1e. Continued partial progress on errors-api-002/003.

Command and network-enabled command execution now share a consuming
StorageExecutionFailure::reject_command method. It performs the same single
cleanup retry, preserves cleanup history before recovering rejected storage,
and returns typed ExecutionOutcome plus the directive index. Exit status,
signal status, setup io::Error and observation io::Error are no longer formatted
away. A failed cleanup retains its full observed history in CommandCleanup.
Successful recovery still returns a build error and discards storage; it never
manufactures an accepted command receipt. A successful command status accompanied
by cleanup failure remains a rejection.

If cleanup remains incomplete, crate-private into_terminal_parts transfers the
outcome/history and drops the recovery owner with existing Drop behavior. This
matches the former terminal string conversion's resource disposition and still
requires disposal of the dedicated VM. It does not detach mounts or certify
quiescence. Lower-level retry APIs remain available before terminal conversion.

Preparation and COPY now use diagnostic history returned after successful
rollback/finalization instead of discarding it; failed attempts also preserve
final history. There is no duplicate primary error string. The original cleanup
sequence, reaper release convention and storage discard rules remain unchanged.
Network setup is still a separate outstanding path using string conversions.

Validation: 143 library tests and strict all-target Clippy passed. Two new tests
cover normal/nonzero/signal exit status with retained cleanup errno and directive
index, and setup errno after clean teardown. Initial pre-test-addition run (141
passed) is preserved. The library run preceded a binding-only correction from
_reaper to wildcard _ to preserve the former COPY release convention; subsequent
Clippy and VM source snapshot include that correction. No new success token or
mutable state flag was introduced.

VM evidence is recorded separately. The first negative invocation had a malformed
image digest and failed input validation; it is not sandbox evidence. The corrected
run targets missing external label policy. Neither run qualifies all platforms,
all cleanup failures, or a full milestone adherence pass. M4 remains 0/3.

Final-source validation: 143 library tests passed after the COPY binding
correction; lifecycle harness compilation also passed. Corrected missing-policy
VM run exited 1 as expected; the maintained verifier passed command-not-admitted,
label denial, command-cleanup-complete and runtime-cleanup diagnostic checks.
Both VM snapshots match all 169 compared source/fixture files. The malformed-pin
run and corrected run are separately preserved with serial/status/source and
hashes of locally retained large assets. Cleanup checks are based on trusted CLI
diagnostics, not independent enumeration of every kernel resource.
