Publication process-death boundaries
===================================
Reproduce from repository root:
  python3 validation/sandbox/guest.py artifact-death \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --out /tmp/NEW-artifact-death --run

Disposable guest and tmpfs workspace; no host mounts/disks/NIC. The runner copies
current Cargo files/src and adds safe-Rust stop gates only in that isolated copy.
helper.rs uses actual Artifact::put/finalize/publish and a small firmware-only
manifest/inventory fixture adapted from tests/artifacts.rs. Canonical inventory
roundtrip is checked before publication. Source and binary hashes are recorded.

Stops occur after the first copied file chunk, after the final cancellation
check immediately before renameat2(NOREPLACE), and immediately after successful
rename but before publish returns. The observer verifies stopped state and stage,
then kills/reaps only that process using its pidfd. It compares every file's
relative path/content hash with the finalized private artifact.

Observed behavior:
- During copy: destination absent; one mode-0700 staging directory remains with
  only a subset of finalized files.
- Before rename: destination absent; one mode-0700 staging directory remains
  containing all finalized file bytes.
- After rename: destination contains all finalized file bytes, no staging sibling
  remains, and the publisher died without returning success.
The original private artifact is unchanged in all cases. The gate wait/reap
bounds are five seconds each; whole guest lifetime is independently bounded.

Evidence: reproduced/artifact-death, including the complete instrumented source
copy. No production changes. Passing assertions describe behavior, including
limitations; they do not mean killed publishers clean up all temporary state.

Implications and limits:
The atomic rename is the visibility/commit boundary. A killed process can leave
an unacknowledged but complete output. Exit failure alone cannot establish output
absence, and an existing destination must not be blindly replaced on retry.
Pre-rename staging leftovers require owner/workspace lifecycle cleanup; this
probe does not implement an adoption or garbage-collection policy. Post-rename
validity here means identical bytes to a finalized fixture, not an assertion
that the guardian remained healthy or attestation was issued.

This does not test power loss, disk durability, parent-directory replacement,
concurrent mutation of trusted inputs, or hardware attestation. copy_tree fsyncs
files and directories before rename; current publish does not fsync the output
parent after rename. No durability claim follows from these tmpfs SIGKILL tests.
The fixture does not run recipe commands or qualify generic executor integration.

Cooperative cancellation extension
----------------------------------
The current helper installs production signal cancellation. Four additional
cases send SIGTERM by pidfd while stopped, then SIGCONT. Copied-source test
instrumentation waits until cancel::check observes the actual handler-set flag
before returning from its gate; timing alone is not used as evidence.

During copy and after copy_tree but before the final check, publication returns
'build cancelled', no destination exists and staging is removed by unwinding.
After the final check but before rename, and after rename, the flag is observed
yet publication returns success and complete output remains. Staging is absent
and finalized private bytes remain unchanged in all cancellation cases. All
three original SIGKILL visibility probes also pass in the same run.

Evidence: reproduced/artifact-cancellation-boundary. This confirms cooperative
cancellation has a final-check/rename window. Another check merely moves the
window; it cannot establish atomic cancellation versus rename. Current behavior
must not be described as 'every cancellation prevents publication'. The design
still needs to define commit/cancellation acknowledgment and guardian-health
requirements. No production behavior was changed and no attestation claim is
made. The injected wait proves flag observation, not production scheduling.
