Publication destination race
============================

Reproduce: cargo test --locked --test artifacts \
  publication_race_preserves_a_new_empty_destination -- --exact --nocapture

A finalized 32MiB repeated-byte PMI artifact uses the real Artifact::put,
finalize and publish implementations. An observer has a ten-second deadline
to observe a nonempty but partial blob in the temporary sibling output.
It then runs a separate mkdir process to create the final destination. Thus
creation occurs after the initial destination-existence check. The directory
is deliberately empty: plain rename could replace it even without deleting
any files. The test requires EEXIST from publish, unchanged device/inode for
the competing directory, an empty preserved destination, no temporary sibling
output, and an intact private finalized inventory.

The normal run passes. A negative control changes only RenameFlags::NOREPLACE
to RenameFlags::empty(), runs the same test, then restores the source in a
finally block. That test fails because publish returns Ok(()), demonstrating
that the test detects the missing atomic no-replace protection. The complete
eight-test artifact suite passes after restoration. Commands, source snapshots,
SHA256 hashes, the negative log and final suite output accompany this record.

Scope: one competing empty-directory creator in a trusted temporary parent.
This is a publication-name collision test, not two full concurrent builds.
It does not prove recovery after process death, protection against a hostile
actor changing ancestor directories, or filesystem crash durability. The
cancellation-check/rename race is a different boundary and remains separate.
No production implementation change was needed for this no-replace behavior.
