# M6 pipeline/output adherence

Scope c0c9ba3; production source remains that validated at 9a70e50. M4/M5 gates
are closed. Three focused self-reviews reuse prior tests and end-to-end evidence;
no new test run or independent security approval is claimed.

1. Forward stage composition: traced Pipeline::execute, DerivedCarapace,
   FirmwareTools and StageOutput. One snapshot spans all carapace directives and
   is sealed once; empty directives create none. Bootstrap and firmware tool
   ancestry stay separate from output ancestry. Firmware extracts both selected
   files from a disposable root and carries the selected output CARAPACE into
   commands/transforms. Trusted COPY uses captured context bytes; commands get
   no context mount. Existing full pipeline proves bootstrap -> multiple RUN /
   COPY -> alternate firmware tools -> DTB binding with exactly two output
   scutes (one bootstrap, one carapace). Owners, immutable plans, consuming
   transitions and safe Rust meet style. No new defect. Clean pass 1.

2. Reverse publication/failure: traced NOREPLACE rename -> inventory/output
   correspondence -> staged assembly -> transform/firmware completion -> runtime
   close. Publication has no fallible operation after successful rename. Output
   graph is reverified from staged blobs, preserves inherited components and
   replaces the firmware pair together. Bad digests/ancestry, changed-DTB
   compatibility errors, failed cleanup or invalid inventory cannot publish.
   An empty input manifest may resolve, but final inventory validation rejects
   an output with neither root nor firmware; this is not a missing output-shape
   check. Partial assembly remains privately staged for cleanup. Existing
   output/inventory/firmware tests cover these boundaries. No new error/style
   defect and no reference loss. Clean pass 2.

3. Requirement/evidence challenge: checked transform semantics (property-only,
   regex replaces all matches, no-match success, nonstring/missing reads fail,
   property-exists condition, authored sequence replaces defaults), template
   handling and CARAPACE availability, inventory v3 and formatter receipts,
   environment-owned networking and no resolver inventory. COPY semantics and
   transform production behavior are unchanged from prior reviewed code;
   current tests/CI cover them. Full pipeline verifier checks canonical CBOR,
   actual root/DTB binding, tool exclusion, produced-scute origin/count and exact
   OCI blob graph. Current network evidence covers enabled/offline selection
   and resolver restoration. Prior failure evidence remains scoped to its
   implementation; no new whole-platform claim follows. M7 must qualify real
   distro builds/boots and available platforms; M8 must preserve and retire
   references. No new finding. Clean pass 3.

M6 closes at 3/3 self-review passes. No source change or redundant validation
campaign was needed to restore this integration gate.
