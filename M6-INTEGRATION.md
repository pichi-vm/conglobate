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
