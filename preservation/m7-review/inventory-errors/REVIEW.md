# Inventory diagnostic finding

Original errors-api-003 in initial-critics.json specifically identified
InventoryError collapsing CBOR decoding and /proc/self/exe I/O into String.
The current action ledger had incorrectly repeated the pipeline remediation
for this distinct finding. Historical evidence is preserved; current evidence
now identifies the actual implementation.

InventoryError retains typed executable I/O, CBOR decoding/encoding and schema
conversion causes. Semantic validation remains Invalid(String): these messages
represent schema rejection, not host I/O. CLI exit behavior and inventory bytes
are unchanged. Private Builder::from_executable consumes a reader and returns a
completed builder identity; it exposes no mutable builder state or new public
construction API. No unsafe code or resource lifecycle changes were introduced.

Tests cover actual malformed wire input, schema mismatch, semantic rejection,
injected executable read failure preserving errno through Error::source, and
successful executable hashing. All 150 library tests and strict all-target
Clippy passed (logs adjacent). Existing canonical inventory tests passed too.
No privileged behavior changed; no new VM qualification is claimed.

This addresses errors-api-003. It is a focused finding check, not one of the
three complete milestone adherence passes. M4–M6 remain reopened, M7 incomplete,
and the broader goal remains active. errors-api-002 retains its separate audit.

The preceding networkd discussion proposes accepting an externally owned
network namespace. That proposal has not been adopted into the implementation
or authoritative design; current per-step networking remains unchanged.
