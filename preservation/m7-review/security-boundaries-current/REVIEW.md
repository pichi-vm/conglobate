# Security boundary finding reconciliation

At d37f1f6 plus one regression test. No production behavior or accepted policy
changed. Original finding fields remain unchanged; current scope matters.

## security-001

revalidated rejection of proposed policy change: Decoded measuring remains uncapped without optional data_blocks; this is real resource use, not denied. DESIGN assigns capacity/resource policy to environment. Requiring absent optional annotation or applying an unrelated filesystem-size constant changes accepted input semantics. No claim of exhaustion resistance.

## security-002

revalidated duplicate of security-001: Materialized data remains RAM-backed and subject to environment capacity. Current single-pass materialization eliminates reread but does not introduce a resource quota. Preserve environment-resource scope; do not claim streaming bounds total allocation.

## security-006

revalidated rejection within binary trust scope: StagedDirectory explicitly provides atomic storage mechanics, not a successful-build receipt. The sole production staged publish call is AssembledImage::publish_inventory after inventory.check_output; private CompletedImage follows context cleanup. Crate publish=false, and image commands cannot call Rust supervisor APIs. This does not certify arbitrary future library callers.

## security-007

revalidated rejection of unsupported invariant: CommandPrivileges clears inheritable/ambient at admission and removes SYS_ADMIN from all permitted/effective/bounding sets. The design requires permanent absence of SYS_ADMIN, not permanent ambient emptiness for allowed capabilities. No filter tightening without workload need; current deny/allow profiles still require distro qualification.

## unsafe-ffi-002

revalidated rejection with regression: Pinned devtree e90bafc Fdt::parse invokes Header::parse which checks >=40 bytes; OwnedTree::encode prefixes writer::build_header [u8;40]. Thus transform offsets 24..32 are bounded by verified dependency invariants. Added test rejects every truncated header length 0..39 in builtin and authored paths; all five transform tests pass.

## security-008

revalidated duplicate of unsafe-ffi-002: Same fixed-header slice; exact pinned dependency parse/encode invariants and new truncation regression are recorded under unsafe-ffi-002.


The dependency source was read from the exact Cargo.toml/lock revision, not a
newer checkout with the same package name. Hashes cover parser, header, writer and
owned encoder. Header::parse checks length before field reads, Fdt::parse calls it
first, and build_header returns a fixed [u8; 40] which encode appends first.
Future dependency updates require this contract review, with the maintained
truncation test as a regression guard rather than a substitute for source review.

Validation: all five transform tests pass, including 80 truncated-header cases
across implicit and authored transforms. Formatting/whitespace checks pass. No
privileged host operations, new VM runs or repeated unrelated suites. Existing
capability and publication evidence retains its original scope; this is not a
new runtime qualification or complete M4 adherence pass. Other findings, UID/GID
contract, distro/platform qualification and final preservation remain outstanding.
