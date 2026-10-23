# Shared inherited DTB capture

Implements accepted simplification-012 after 53d6b9b. CheckedImage now owns the
crate-private inherited_dtb operation. InheritedBuild and CarapaceBuild both use
it, eliminating duplicate manifest selection and verified metadata capture.

The method selects only the DTB descriptor, parses its digest, and uses the
cache metadata reader with the descriptor size. It deliberately rechecks bytes;
previous input receipts are not treated as authority for subsequent cache reads.
Absent DTB remains None, malformed digest/read failure remains CacheError, and
DTB transforms still receive the selected output root from their own stage.
No firmware-source selection or component propagation semantics changed.

Validation: 126 library tests plus 13 context/input tests passed. Existing output
assembly tests cover inherited/transformed component graphs; context tests cover
resolved input selection. Formatting and git diff --check passed. No new tests
were added solely to mirror this extraction. No VM run or full milestone pass
is claimed. Historical sources and evidence remain untouched.
