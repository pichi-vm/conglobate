# Milestone 3 adherence evidence

Status: complete; clean streak three. These are source-grounded self-reviews,
not independent reviews. Milestone 4 may begin; its runtime/security requirements
remain unimplemented and unqualified.

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

## Full-pass attempt 1 (baseline 2d9d383): finding, not clean

Requirements-to-parser traversal found that filesystem properties used a
plain defaultable mapping. serde-saphyr accepted explicit `properties: null`
as an empty mapping. A new regression failed on that exact input. Optional
properties, label and derive fields now use the same explicit Presence decoding
as stages: omission supplies defaults; explicit null fails; `{}`/`[]` remain
valid where appropriate. Tests also reject null derive lists and null labels.

Reviewed raw/validated separation, required CLI selections, top-level lock pins,
stage combination rules and typed transform parsing before this finding stopped
the clean-pass attempt. No success is inferred for unreviewed categories.
After the fix, all 52 unit/integration tests, three compile-fail doctests and
Clippy pass. Only recipe parsing/tests and this evidence changed; preservation
references and runtime mount implementation are unchanged. Streak remains zero;
the next full pass must use the corrected scope.

## Clean pass 1 — requirements-first, scope 621b34d

Reviewed the active library, tests and Cargo configuration at 621b34d against
DESIGN.md sections 2–8 and the M3/M4 boundaries in the architecture and milestone
ledger. No implementation or test changes were made during this pass.

1. Requirements: traced required CLI selections through committed document
   capture, strict recipe parsing, fixed stage order, role aggregation and locked
   image resolution. Bootstrap/alternate firmware tools do not become output
   ancestry; authored transforms require an actual inherited or prospective DTB.
   Empty/null/omitted distinctions, finite timeouts, offline defaults, resolver
   requirement and deferred template evaluation agree with the current design.
   COPY modes/patterns are validated descriptions; filesystem COPY and transform
   execution remain explicitly M6, not implicitly claimed as working here.
2. Rust style: inspected constructors, private fields, borrowed views and
   consuming capture/resolution/mount transitions. Checkout states are sealed
   markers with distinct owned payloads. Production behavior resides in methods;
   no freestanding utility-function architecture or shared mutable state was
   introduced. Optional values represent optional inputs/components; MountLease's
   Option is the documented destructor ownership slot, not a runtime state model.
3. Safety/recovery: traced Git object verification and confined link reads;
   pinned cache descriptors, bounded metadata capture, exact streaming lengths,
   decoder EOF and ancestry checks; and checkout creation/remount/unmount failure
   paths. Staged readers cannot escape their owner. Explicit unmount failure
   retains recovery authority. Drop is only a fallback, not a crash guarantee.
   Cache receipts prove completed reads; later mounted reads still need verity.
   Private namespace establishment and durable recovery anchors remain M4 gates.
4. Evidence: reviewed positive/negative assertions in recipe, plan, template,
   context, cache, chain and COPY tests, including independent salted hash vectors
   and invalid zstd streams with correct OCI digests. Re-ran all 52 tests and
   three compile-fail doctests successfully; fmt and Clippy -D warnings pass.
   VM v6 evidence covers actual read-only enforcement, EBUSY retry, Drop and
   partial-setup cleanup. Its verdict explicitly does not qualify the full sandbox.
   No redundant VM run is claimed for recipe-only changes since v6.
5. Preservation/scope: working tree was clean; active source/tests were unchanged
   from the scope commit. Validation changes since the preservation baseline are
   additive; old implementation remains under reference/legacy. No references
   were retired, no CI or distro/security qualification is inferred, and no M4
   implementation work was started.

Result: clean, streak 1/3. This is a source-grounded self-review. Test execution
supports the result but is not itself the adherence pass. Two further complete
passes over the same implementation scope are required before advancement.

## Clean pass 2 — failures/evidence first, scope 621b34d

Started with the VM fixture and regression assertions, then traced their failure
conditions back through production APIs and the architecture. Only ledger changes
exist since the implementation scope; no source or test edits during this pass.

1. Evidence: the production VM fixture checks EROFS, holds a file across an
   unsuccessful normal unmount, retries through the returned recovery owner, and
   exercises post-remount EMFILE using a restored descriptor limit. These are
   actual library calls, not mock lifecycle state changes. Re-ran compile-fail
   doctests with diagnostics visible: E0505 for the live-reader transition,
   E0599 for freezing ReadOnly again, E0277 for moving the mount lease to a
   thread. They fail for the intended restrictions, not unrelated syntax/import
   errors. Examined independent chain vectors and decoder-error fixtures; the
   prior full-suite/fmt/Clippy results still apply to unchanged source/tests.
2. Safety/recovery: followed each fixture into MountLease and consuming
   transitions. A failed close preserves the lease; a failed post-bind operation
   cannot return a ReadOnly checkout. Before-bind errors own no mounted resource.
   Explicit successful close drops the root FD before unmount. Descriptor and
   mount views are not interchangeable, and the API/VM test account for that.
   Checked cache errors through exact stream completion and ancestry traversal:
   no receipt or ResolvedBuild is returned after a digest, length, decoder,
   missing-parent or duplicate-root failure. No command admission exists yet.
3. Requirements: traced rejected requests/recipes and all eight stage combinations
   to DESIGN.md. Role aggregation preserves separate input purposes and implicit
   firmware tool requirements. The inherited-DTB preflight fixture demonstrates
   component presence, not valid final output shape or firmware pairing; those
   remain M6 validation obligations. Resolver presence is preflight; capture and
   isolation are later setup work. Undefined templates fail when evaluated,
   while parsing preserves deferred CARAPACE availability.
4. Rust style: reviewed owning error variants, private validated fields,
   sealed states, consuming transitions and read-only getters. Raw recipe types
   cannot construct verified image/mount authority. Parser accumulators and
   stream counters are local algorithm state, not mutable build-wide state.
   Test entrypoints and the validation executable are justified freestanding
   harness functions. No application unsafe code or writable handle escape added.
5. Preservation/scope: confirmed only the two adherence ledgers changed since
   621b34d, no baseline validation files were deleted, and the baseline manifest,
   archive locator and legacy tree remain. No temporary external evidence was
   retired. Public library documentation still says execution is unimplemented;
   this pass does not widen claims to the full sandbox, distros or CI.

Result: clean, streak 2/3. One further complete pass is required. No milestone-4
advancement follows from this record.

## Clean pass 3 — public authority and acceptance audit, scope 621b34d

Started from exported constructors/getters and the M3 acceptance criteria, then
checked their implementation, error ownership and supporting evidence. Source,
tests, dependencies and VM harness are unchanged across all three clean passes.

1. Rust style/API invariants: audited the exported type/method surface. Parsing
   identities cannot manufacture verified metadata, chain receipts, captured
   contexts or mounted checkouts. Those types have private fields and no public
   Deserialize/Clone authority escape. Public recipe enums are read-only views of
   a privately constructed Recipe; callers cannot mutate the plan through them.
   RelativePath distinguishes file/directory syntax without claiming resolution
   authority. Checkout state traits are sealed and their opaque payloads own
   resources. ReadOnly returns an actually read-only file; StagedFile exposes
   neither raw descriptors nor mutation methods. Methods, consuming conversions
   and local parser/stream state follow the agreed Rust preferences.
2. Requirements/acceptance: checked the complete M3 boundary:
   - new safe crate and explicit recipe/lock CLI parsing: present;
   - selected committed bytes and their path/content identities: captured;
   - confined private tmpfs materialization and read-only transition: implemented
     and exercised in the disposable VM;
   - local index pins, host selection, all referenced blobs and scute ancestry:
     verified with error propagation, no image fetching/cache mutation;
   - single recipe, fixed stage order, input roles/from restrictions, retained
     base metadata and omitted/empty transform selection: represented without
     treating tool inputs as inherited output;
   - meaningful positive, negative and ownership tests: present and passing.
   Retained components are inputs for later composition, not an emitted artifact.
   No empty-manifest fixture is claimed to prove valid output publication.
3. Safety/recovery: checked authority across transitions against ARCHITECTURE.md.
   Git access ends after capture. Cache handles are read-only/confined; captured
   metadata is distinct from stream receipts and later mount verification.
   Partial mounting retains an owning error; successful explicit cleanup and
   best-effort Drop are distinct. The existing public mount transition has a
   trusted private-namespace precondition; M4 must establish that namespace and
   the recovery protocol before integration permits commands. No claimed safety
   depends on a destructor surviving process death.
4. Evidence/claims: reviewed the test-to-requirement mapping and the three
   compile-fail diagnostics from pass 2. The unchanged scope retains the passing
   52-test/fmt/Clippy baseline. VM v6 serial output includes all three probe
   markers and exit 0; the preserved probe binary SHA256 matches its input
   manifest. This verifies recorded fixture provenance, not confidential hardware
   support. The library explicitly documents execution as not implemented.
   CI restoration, command isolation, output artifacts and distro qualification
   remain later milestones and are not counted as M3 results.
5. Preservation/scope: independently compared 40 retained legacy files having
   baseline SHA256 entries with their current bytes: all match. Original tracked
   validation is unchanged outside the two additive rewrite validation folders.
   The implementation/VM-harness diff from 621b34d is empty. Archive/source
   retention and final removal gates remain intact; no references were deleted.

Result: clean, streak 3/3. Milestone 3 is accepted at its foundation scope.
Milestone 4 may now start. A later discovered foundation defect reopens this
gate as required by the goal; acceptance is not final sandbox qualification.
