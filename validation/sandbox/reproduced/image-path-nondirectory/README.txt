Image path non-directory traversal regression
=============================================
The baseline resolve implementation accepted /file/../resolver and a symlink
with relative target file/../resolver despite file being a regular file. An
interactive regression run returned Ok with the resolver path instead of an
error. The baseline failure log was not separately retained.

The correction rejects an existing non-directory component when more path
components remain. The test compares the error against actual kernel metadata
lookup for both direct and linked traversal: all return NotADirectory. Existing
absolute image-link confinement/cycle tests still pass. The copy test target
reports two passes and three root-required ignored tests; those ignored tests
are NOT claimed as checked here. All twelve library tests pass, including COPY
account metadata and export path/type checks. Commands, outputs and source
snapshots/hashes accompany this record.

This is a production path-resolver correctness fix, not an arbitrary resolver
injection implementation. Missing final targets remain permissible to resolve
because COPY can create them; a resolver-injection caller must separately
require an existing regular file. Path walking requires all recipe writers to
be stopped, as documented. No concurrent path-mutation safety is claimed.
