# Accepted API simplification batch

Addresses ownership-008, simplification-017 and simplification-027 after baseline
a4c6ce6. These were accepted historical actions, not newly demonstrated escapes.

- DtbContract selects at most two matching fills with an iterator and destructures
  exactly (Some(fill), None). Zero or multiple matching fills retain the same
  rejection. Manifest decoding/duplicate-key checks still examine the manifest;
  only the temporary matching-fill vector is removed.
- Removed SealedScute::hash_reader and its hash_size bookkeeping. The actual hash
  scratch, sync, verity generation and internal files() consumer remain. No
  writable public access is added. Source search found no hash_reader callers
  in current source/tests or historical harness source; archived definitions
  remain unchanged as evidence.
- Removed only FormattedSnapshot::format. FormatReceipt::format is used by the
  inventory and historical formatter validation and remains. Other format()
  calls create filesystems/verity, not calls to the removed accessor.
- Validation: cargo +stable test --lib: 126 passed. Maintained lifecycle crate
  cargo check passed. cargo fmt applied; git diff --check passed. Logs retained.
  Tests cover missing/duplicate DTB fills, snapshot metadata and publication.
- No VM run or platform qualification inferred. Public crate is unpublished;
  preserved implementations are reference inputs and not modified to hide API
  history. This focused check is not a full milestone adherence pass.
