# Unused API and infallible encoding cleanup

At 9d3fc1a, three accepted findings still applied.

- simplification-018: removed SnapshotUsage total_sectors/metadata_sectors
  accessors. No active, maintained or non-archived reference harness caller was
  found. All counters remain in the value, and TryFrom still rejects zero total,
  metadata above allocated, allocated above total and exceptional device states.
- simplification-022: removed AssembledImage::inherit, used only by its own two
  tests. Tests now invoke the production transformed path with an unchanged,
  parsed DTB. Exact original manifest bytes and self-contained exported content
  are still checked after loss of the source cache.
- errors-api-010: network configuration CBOR encoding now uses expect("Vec
  writer") rather than bare unwrap for minicbor's infallible Vec writer. The
  encoding schema, bytes and decoder are unchanged.

The first migrated missing-reference test failed: it removed the original DTB
blob but also supplied captured DTB bytes, so transformed correctly succeeded.
This was a fixture assumption inherited from the deleted wrapper, not evidence
that assembly must reread an already captured value. Preserved the failure log.
The missing-reference fixture now removes the PMI blob, which assembly must
still read. Added a separate assertion that captured unchanged DTB bytes survive
loss of their cache copy, with complete exported-reference verification and
explicit staging disposal. No production assembly semantics were changed.

Final validation: 131 library tests, all-target strict Clippy and maintained
lifecycle crate check passed. Source hashes and initial/corrected test logs are
preserved. Historical source and archived fixture callers remain recoverable;
they were not rewritten to erase the old API.

Focused review only. No VM/platform qualification or complete milestone
adherence pass; M4 remains 0/3.
