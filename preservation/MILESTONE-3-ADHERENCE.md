# Milestone 3 adherence evidence

Status: incomplete; clean streak zero. These are source-grounded self-reviews,
not independent reviews. No milestone-4 advancement is authorized by this record.

## Initial requirements review (baseline 6326d48)

Scope inspected: committed context capture, subtree selection, confined reads,
materialization, path values and template validation. This is a partial review,
not a completed pass over the milestone's required categories.

Findings:

1. Ancestor trees traversed to select a context subtree were hash-verified but
   not checked for duplicate names. Selection used the first matching entry,
   allowing an ambiguous committed tree to choose a context. Fixed by validating
   names and uniqueness whenever ObjectReader obtains a tree, including ancestors.
   Regression uses a test-owned Git repository with a duplicate-entry tree.
2. In-memory symlink resolution discarded the directory requirement of a trailing
   slash or slash-dot. A link to a regular file ending in either could succeed
   during recipe capture but fail during kernel-backed materialized reads. Fixed
   by retaining a directory requirement during captured resolution. Regression
   tests both captured and materialized reads for agreement.
3. Read-only lifecycle enforcement remains unproved. StagedContext is a private
   tmpfs checkout with read-only public file opens, not a read-only mount. The
   architecture assigns mount lifecycle to M4 while the M3 criterion names a
   read-only checkout. This boundary must be resolved with an actual enforcing
   transition and evidence before the M3 gate is claimed; do not silently count
   restrictive staging permissions as a read-only mount.

Validation after fixes 1–2: all 50 tests pass; cargo fmt and Clippy with warnings
 denied pass. Runtime sandbox, read-only mount, crash recovery and distro claims
remain unsupported by these tests. Existing source/validation preservation is
unchanged. No clean pass is counted while review scope and finding 3 remain open.
