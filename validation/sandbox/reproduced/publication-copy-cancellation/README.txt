Publication-copy cancellation
=============================

Reproduce: cargo test --locked --test artifacts
The dedicated test forks itself to isolate the irreversible process-wide
cancellation handler. The captured command runs that child directly to retain
its observed byte count. Fixture: real Artifact::put, finalize, and publish;
128MiB repeated-byte PMI blob with a matching inventory/manifest. A trusted
observer thread polls the temporary sibling output for a nonempty but partial
copy of that blob, then raises SIGTERM. No arbitrary sleep controls the signal.
Observation has a 10-second deadline; missing the partial-copy window fails the
test rather than substituting a pre-publication signal. The captured run also
has an external 60-second timeout.

Result: publication returns "build cancelled"; destination is absent; the only
remaining entry is the private finalized artifact (normal workspace ownership).
The temporary sibling copy is removed. All seven artifact tests also pass.

Implementation change: copy_tree previously used std::io::copy for an entire
file, so cancellation checks occurred only between entries and before rename.
It now reads/writes at most 64KiB between cooperative cancellation checks.
This is source-grounded behavior improvement; no old-code timed failure or
throughput comparison is claimed. The integration test proves cancellation
observed during a partial copy rejects publication and cleans sibling staging;
it does not measure an exact bound on bytes copied after the signal.

Limitations: a blocked read/write/sync syscall is not interrupted by a flag
check. SIGKILL/process crash cleanup is not covered. Cancellation can arrive
between the final check and rename; an atomic rename is the visibility boundary,
not an atomic transaction with the cancellation flag. Successful rename can
therefore win this race. No rollback, crash durability or full supervisor-death
claim follows from this test. Kernel/filesystem/backend failure paths and
concurrent cleanup ownership remain separate validations.
