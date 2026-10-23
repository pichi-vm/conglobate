# Inventory publication simplifications

Source review confirms every AssembledImage constructor reaches resolve_staging,
which writes and verifies index.json and oci-layout before constructing the owner.
publish_inventory now adds only inventory.cbor, after check_output, and publishes
through the existing atomic no-replacement operation. It no longer rewrites the
verified layout. The low-level method is crate-visible. This resolves historical
simplification-023 without moving serialization into storage.

Removed Inventory::sha256 and its sole caller, the redundant roundtrip assertion.
Exact encoded byte equality remains tested; captured input SHA-256 identities and
builder/output digests are unchanged. This resolves simplification-028.

Validation: cargo +stable test --locked passes 232 tests and 53 doctests;
strict all-target Clippy and formatting pass. Raw logs are retained here. Existing
publication-parent, output-layout, inventory roundtrip and pipeline tests exercise
the affected paths. No new mirror-of-implementation test was added.

These are M6 changes; its complete adherence gate remains 0/3. They do not complete
M4, M5, distro/platform qualification, or reference retirement. Previous source
and review findings remain in Git history.
