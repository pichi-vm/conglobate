# M6 pipeline and publication

M5 is accepted on preservation/m5-review/scope-v2.json with three clean
self-review passes. M6 is active with zero passes. DESIGN.md and
SANDBOX-DESIGN.txt remain authoritative; this is an integration order, not a
replacement specification or a narrower acceptance boundary.

## Starting interfaces and missing connections

CapturedBuild owns committed context, selected recipe/lock identities and the
immutable BuildPlan. ResolvedBuild owns local Cache and CheckedImage inputs;
resolution verifies references/chain and rejects missing execution carapaces.
These are implemented gates, not a runnable CLI. BuildRequest parses explicit
context-relative recipe/lock paths but the crate has no executable entrypoint.

M4/M5 supply actual preparation, execution receipts, finalization and direct
sealing for one or two volumes. Their integration fixture currently supplies
orchestration and OCI metadata. Production COPY has pattern/mode descriptions
only; transforms have parsed operations/templates only. Network-enabled steps
are identified in the plan and require a resolver argument, but enabled network
setup is not connected. No production inventory/publication coordinator exists.

## Integrated implementation order

1. Connect resolved inputs to an owned output-image assembly and production OCI
   publication. Start with the valid pass-through recipe, preserving every base
   component and selecting only the host manifest. Keep execution tool inputs
   separate from output ancestry. Capture/reverify actual blob bytes used for
   publication; do not trust earlier verification of a mutable cache pathname.
   Include canonical empty config and every referenced blob. Generate deterministic
   version-two inventory from actual identities and receipts; final validation
   cross-checks it against the final manifest. Stage under a temporary sibling and
   atomically rename without replacement. Exercise through a real CLI invocation
   on a committed recipe/lock and verify the emitted layout independently.
2. Connect the same coordinator to M5 bootstrap and single-volume carapace
   execution, then confined COPY. Preserve per-directive order, per-run remounts,
   finite timeouts and original cleanup deadlines. Empty derive/pass-through must
   not invent a layer. Use sealed output directly as later-stage origin; do not
   publish intermediate artifacts or convert a filesystem image. Every error
   retains required cleanup authority and prevents final publication.
3. Add firmware execution/extraction and DTB transformation to this same output
   assembly. Firmware tools may differ from the output base and never become
   ancestry. Extract both regular PMI/DTB outputs through confined handles only
   after writers stop, validate them, and replace the pair together. Apply safe
   DTB property edits and built-in corium behavior to the final DTB. Preserve
   unchanged properties, reservation entries and header semantics. Expose the
   output root as CARAPACE, not the alternate firmware tool root. Record actual
   effective transform identity and skip status.
4. Wire enabled networking through owned setup/teardown into the existing command
   lifecycle. Disabled stays the default with a separate network namespace.
   Enabled gets outbound NAT/replies, all reachable destinations and explicit
   resolver bytes at a root-confined destination; no resolver inventory entry or
   persistent image injection. Setup/teardown have fixed internal deadlines.
   Use benign disposable-VM connectivity/DNS/cleanup fixtures only.
5. Close the complete stage/output matrix and failure regressions, including
   standalone CLI helper dispatch, no downloads, absence versus explicit empty
   transform, firmware-only output, inherited unknown components, cache mutation,
   output collision, interrupted staging, missing artifacts, cleanup failure and
   no publication after failed commands. Verify OCI interoperability with ORAS.
   Then perform the three complete adherence passes on a frozen scope.

## Ownership requirements

The coordinator owns immutable request/resolved inputs separately from active
resource sessions. Output assembly carries content provenance without ambient
path authority. Execution receipts, clean storage finalization, sealed layers
and publishable output remain distinct types; no public constructor converts a
failed command into publishable state. Use sealed marker states where phases
change available operations, consuming conversions, RAII retention and explicit
fallible cleanup. Avoid a mutable global build-state bag and duplicate single/
bootstrap execution state machines. Safe libraries own wire parsing.

All listed work is required before M6 acceptance. Package-manager builds and
boots for Fedora/Debian/Alpine/NixOS, additional architecture/confidential hardware
and full repository qualification remain M7. Preservation retirement remains M8.

## Output storage transaction implemented

StagedDirectory owns a private temporary sibling anchored to a parent descriptor.
It copies cache blobs through Cache::copy_verified, checking the exact written
stream and only installing a digest-named blob after successful verification.
The existing verify method shares that stream validation through a sink. Failed
copies discard their private temporary file. Captured/produced small bytes can
be stored by digest; metadata writes have fixed filenames. Publication uses
renameat2 NOREPLACE and returns the staging owner on failure. It performs no
fallible work after a successful rename, avoiding ambiguous publication errors.

This is a low-level filesystem transaction, explicitly not a build-success or
validated-inventory receipt. The coordinator/assembler still must validate the
manifest, inventory and complete referenced blob set before calling it. No CLI
build or complete OCI artifact is claimed yet.

Three output tests pass (late collision preservation, cache mutation rejection,
and parent rename with exact output bytes); all 12 cache tests pass including
writer failure propagation. Strict all-target Clippy passes. Source/log hashes
are in preservation/m6/output-storage. Tests use ordinary temporary directories,
no host mounts or policy loading. M6 remains at zero adherence passes.

## Inherited artifact assembly

AssembledImage::inherit creates fresh staging and copies the selected manifest's
configuration and every layer through verified streams. It preserves exact
manifest JSON bytes, including fields the shared typed view does not expose.
Canonical inline empty configuration is materialized as a blob. The output index
selects only the host manifest. Reopening staging through Cache verifies the
manifest, all referenced bytes and the carapace chain before returning an
immutable assembled owner. No mutable staging handle or publication method
escapes this owner; inventory validation/publication remains to connect.

Two new cases pass: both real and inline-empty configuration remain self-contained
with PMI/DTB after deleting the source cache, including exact manifest extensions
and no unreferenced index blob; a missing reference retains private staging for
explicit cleanup and never creates the output destination. The firmware bytes
are benign opaque fixtures: these tests prove transfer/content identity, not PMI
or DTB semantic validation. All five output tests and strict all-target Clippy
pass. Initial constructor-refactor compile failure and corrected logs/source are
preserved under preservation/m6/inherited-assembly. M6 remains zero clean passes.

## Version-two inventory wire implementation

Inventory is an immutable validated document with its exact canonical CBOR bytes
and digest. Private typed records include single recipe/lock paths, formatter
receipts, pipeline network identifiers and transform skip status. Ciborium and
serde_bytes provide safe serialization; a consuming value normalizer orders map
keys by encoded bytes and rejects duplicate keys, tags and floats. Strict decode
checks both canonical wire bytes and typed-schema roundtrip, rejecting unknown
fields, unsupported versions, nonminimal/indefinite encoding, trailing data,
optional nulls and alternative representations. Enum/byte-array types and
cross-field validation constrain roles, identities, paths, roots, output pairs,
formatter geometry/runtime order and network-step order.

This validates inventory representation, not authenticity or actual execution.
The private record constructor is not yet connected to completed stage receipts
or AssembledImage. Manifest/inventory cross-checks and the CLI remain next.
No attestation or resolver measurement was added. Five inventory tests pass,
including full formatter/network schema and negative wire corpus; all 66 library
tests and strict all-target Clippy pass. The command fixture compiles with its
updated dependency lock. Logs/source hashes are in preservation/m6/inventory-wire.
No new VM run was needed for this unprivileged serialization change. M6 zero passes.
