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

## Read-only transition follow-up

The production StagedContext::read_only transition now replaces the pre-bind root
FD, owns the read-only mount, and retains cleanup authority on failed explicit
unmount. Drop uses a detach fallback and retains backing storage if detach fails.
The mount lease is thread-affine. StagedFile is borrowed and exposes no raw FD or
metadata mutation API; a compile-fail test prevents consuming the staged checkout
while a reader remains alive. This closes the prior escape of pre-bind descriptors
through StagedContext::open.

Disposable-VM runs v3/v4 in validation/rewrite-context-mount exercise production
read-only enforcement, EBUSY ownership/retry and Drop cleanup. This supplies the
missing ordinary read-only checkout transition evidence for finding 3. Private
namespace establishment, simultaneous crashes and broader lifecycle recovery are
still M4 requirements. Setup fault injection and the full milestone-wide review
remain to be considered; no clean pass has yet been counted.

## Pipeline requirements review (baseline 4997ee8)

Reviewed input resolution, stage/reference planning, optional stage parsing,
transform selection and timeout values against DESIGN.md sections 5–8. Found
that an authored transform (including an empty list) could pass preflight with
no possible output DTB. This violated the explicit empty-versus-omitted rule.
Resolution now requires a DTB from the output base unless firmware will produce
one. Tool images never supply inherited output DTBs. The implicit default still
skips when no DTB exists. Added regression cases for omitted, empty and nonempty
transform selections, and a positive case using an inherited empty DTB.

Affected 12 context/input integration tests pass and Clippy passes for all
 targets. This is a review with a fixed finding; clean streak remains zero.
The rest of the milestone-wide review and setup-failure evidence remain pending.

## Cache error-path and setup-recovery review (baseline c42e2c5)

Inspected CheckedBlob, DecodedBlob, ScuteExtent integration, ancestry traversal,
and read-only mount owner/drop paths. Added missing tests to distinguish decoder
validation from OCI hashing: checksum-corrupt, truncated and trailing-junk zstd
streams have matching descriptor digests but must still fail; valid concatenated
frames succeed. All 11 cache tests and Clippy pass.

Production setup failure is now exercised in disposable-VM v5 by exhausting the
probe's descriptor soft limit after staging. Bind/remount succeed, reopening the
read-only view fails with EMFILE, and the returned owner retains the mount until
cleanup retry after limit restoration. This closes the ordinary partial-setup
coverage gap without adding production failure hooks. Previous EBUSY retry and
Drop paths also pass in that run. Simultaneous crashes are still an M4 obligation.

This expands the evidence scope, so it does not count as a frozen-scope clean
pass. No new production-code defect was found in this portion of the review.

## Rust state-model review (baseline 8a4efcc)

Found a style/architecture discrepancy: staged/read-only checkout transitions
used separate concrete wrapper structs rather than the agreed sealed marker
state methodology. Refactored to Checkout<Staged> and Checkout<ReadOnly>, with
sealed CheckoutState, empty markers and associated opaque resource payloads.
StagedContext/ReadOnlyContext remain aliases; resource guards retain their RAII
and explicit cleanup behavior. No unsafe extraction or runtime state flag was
introduced. MountLease's single Option remains solely a Drop ownership slot.

All 51 unit/integration tests, three compile-fail doctests and Clippy pass.
Negative compilation checks cover freezing with a live reader, freezing twice
and sending mount ownership across threads. Disposable-VM v6 repeats normal,
EBUSY retry, Drop and actual partial-setup cleanup successfully. This fixes a
review finding and changes the reviewed scope, so the clean streak remains zero.
