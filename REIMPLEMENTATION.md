# Conglobate reimplementation goal

Status: active. No milestone may advance until its three-pass gate is clean.
The target is the latest DESIGN.md, SANDBOX-DESIGN.txt and user decisions.
This ledger describes required work, not claims about the current executor.

## Milestones and acceptance criteria

| Milestone | Deliverables and success criteria | Status | Clean streak |
| --- | --- | --- | --- |
| 1. Preserve and separate | Preserve full working-tree implementation, uncommitted changes, tests and all validation sources/evidence; verify archive hashes; record Git baseline and external-artifact dependencies; keep recoverable originals before moving old code outside the new active build. | Complete | 3 |
| 2. Architecture contract | Define owning modules/types, sealed state traits, consuming transitions, fallible finalization and recovery, confined filesystem authority, typed formatter properties, inventory migration, and requirements-to-tests traceability. Identify external qualification requirements without claiming access. | Complete | 3 |
| 3. Inputs and planning | New safe Rust crate; required context-relative recipe/lock paths; exact committed context and local verified image references; one recipe mapping, immutable plan, complete stage validation/inheritance; meaningful positive/negative tests. | Complete | 3 |
| 4. Privileged lifecycle | RAM workspace, cache-backed verified layers, kernel/idmapped mounts, scoped LSM, no command SYS_ADMIN in any capability set; private namespaces, restricted descriptors/syscalls, bounded execution/setup/teardown, cancellation and descendant quiescence; partial-setup/recovery tests and no context/control/block-device exposure. | In progress | 0 |
| 5. Direct-layer bootstrap | Trusted formatting on writable snapshot over dm-zero; tool / and output /mnt; per-step remount and policy for both; correct persisted metadata; actual carapace storage chain, clean sealing, no image conversion; failure tests. | Pending | 0 |
| 6. Full pipeline and publication | Layering, confined COPY, firmware pair extraction, DTB transforms/defaults/CARAPACE; offline default plus outbound NAT and explicit DNS; versioned deterministic CBOR inventory, self-contained OCI and atomic no-replacement output; maintained regression and integration coverage. | Pending | 0 |
| 7. Integrated qualification | Critical source/design/style review resolved; Fedora/Debian/Alpine/NixOS representative builds and boots; amd64/arm64 and TDX/SEV-SNP/CCA qualification with evidence; repository checks and CI green. Unavailable required environments remain outstanding, never passes. | Pending | 0 |
| 8. Retire references | Historical findings migrated to maintained tests/docs; remove legacy implementation and exploratory validation from active repo only after recoverability verified and replacement evidence accepted; final coherent docs/code, tests and three adherence passes. | Pending | 0 |

## Mandatory adherence gate

After each milestone, conduct three distinct, consecutive source/evidence review
passes against the frozen milestone contents. No subsequent milestone work until
all three are clean. Every pass covers every category below; use a different
reading order or starting point to avoid simply repeating the same inspection.
These are self-review passes unless an independent reviewer is explicitly used;
never describe them as independent reviews. Tests supply evidence but repeated
test runs do not count as adherence passes.

1. Requirements: trace agreed design and latest user choices to implementation,
   documents and tests; reject contradictions, accidental scope changes and
   unsupported claims.
2. Rust style: methods on meaningful objects; no utility-function architecture;
   freestanding functions only where justified (entrypoints/test harnesses);
   sealed marker-trait typestate; private invariant-bearing fields; RAII resource
   ownership; immutable validated descriptions and consuming conversions;
   avoid runtime state flags/option bags and unnecessary shared mutability.
3. Safety: forbid unsafe application Rust, safe library interfaces, narrow
   authority, resource/identity lifetimes, explicit fallible completion,
   cancellation/partial failure and recovery; no destructor-guarantee assumptions.
4. Evidence: relevant positive/negative/failure tests, actual emitted artifacts,
   provenance, meaningful assertions, coverage boundaries and current CI/checks.
5. Preservation/scope: no lost user work; references preserved until final
   migration; no unexplained generated artifacts or destructive cleanup.

Record scope identifiers, observations, findings, fixes and pass result. Any
finding or scope-changing edit resets the streak to zero. Fix and rerun affected
checks before starting a new sequence. Ledger-only recording of a clean review
does not alter its reviewed implementation scope. Later discoveries reopen the
affected milestone and block dependent advancement until its gate is restored.

## Execution policy

Work autonomously in coherent reviewable commits. Do not push until publication
is appropriate to the agreed CI work. Ask only for decisions changing requirements
or genuinely unavailable inputs. No exploit code: security checks use owned,
benign fixtures in disposable environments. Never load experimental policy or
mount fixture filesystems in the host kernel.

Preservation includes failures, patches and evidence, not just passing reports.
Generated binaries/caches may be preserved in verified archives rather than
ordinary source commits. Keep their manifests and locations explicit. Material
referenced outside the repository must be inventoried before final retirement.
Final retirement must retain recoverable source/evidence history and explicitly
account for archived bytes; a temporary pathname alone is not durable provenance.

A completed milestone is not production security approval. No goal completion
while required implementation, qualification, final cleanup or adherence gates
remain outstanding. Report environmental blockers honestly; follow goal-status
rules rather than silently shrinking acceptance criteria.

## Milestone 1 work log

- Baseline HEAD: d663e7a (build: update carapace and devmap revisions).
- Working tree contains existing modified production files and untracked design
  and validation material. Preserve exact bytes before moving any files.
- Validation is approximately 4.8 GiB, including nested compiled-helper caches.
  Root target/ is a separate approximately 24 GiB generated build cache.
- Preservation destination: sibling conglobate-preservation/2026-10-22/.
  Archive verification and separation are in progress; no adherence pass yet.

- M1 completed: preservation tag rewrite-reference-2026-10-22 at 7bd1863;
  full archive verified; complete Git bundle verified; 12,538 Git archive files
  checked against baseline; 35 relocated implementation files checked exactly.
- Reference tree: reference/legacy/. Validation remains at validation/.
- Three clean adherence passes recorded in preservation/MILESTONE-1-ADHERENCE.md.
  Architecture work may now begin. No production rewrite code exists yet.

## Milestone 2 work log

- Draft ownership/module/state/error/recovery architecture in ARCHITECTURE.md.
  It is explicitly not through the adherence gate; no new crate implementation
  has started.
- Current pinned carapace 99caf79 exposes read-only Chain::assemble through
  Assemble; its internal zero origin is 64 GiB (ZERO_COUNT_SECTORS). Requested
  bootstrap filesystem capacity must be reconciled with this geometry rather
  than silently changing the carapace wire contract. Inspect assembly/devmap
  ownership APIs next, then settle formatter geometry and recovery design.
- Architecture clean streak remains zero while these design details are open.
- Architecture refinement: filesystem capacity is distinct from carapace's
  fixed 64-GiB geometry; explicit formatter block count avoids wire changes.
  Initial size syntax/label limits and recorded formatter profile are specified.
- Safe integration gap identified: cached rustix/nix lack mount_setattr. M4 must
  use/contribute a qualified safe library interface, not application unsafe FFI.
- Recovery protocol now requires persistent kernel-held namespace/filesystem/
  policy anchors, registered command containment and guardian coordination.
  Simultaneous guardian/supervisor loss retains identities for later recovery.
- API feasibility probe: safe state-specific resource ownership and recoverable
  transition error pass; invalid borrow/state clients fail E0505/E0599.

- M2 accepted after initial findings/fixes and three clean adherence passes:
  preservation/MILESTONE-2-ADHERENCE.md. Architecture scope is accepted, not kernel
  implementation qualification. The safe idmapping dependency and actual recovery
  protocol are explicit M4 gates. M3 may begin.

## Milestone 3 work log

- Created a new library crate from scratch with forbid(unsafe_code), private
  invariant-bearing fields and object/trait methods. No legacy source copied.
- Implemented context-relative lexical path values (explicitly not filesystem
  confinement), filesystem capacity/block-count and label values, finite command
  timeout values, image names/index digest values, strict YAML policy and immutable
  top-level image locks. Duplicate mapping keys/fields and unknown lock fields fail.
- Six behavioral tests pass; cargo fmt and Clippy with -D warnings pass. The
  first doctest launch hit the host /tmp quota; rerunning all checks with
  TMPDIR=/var/tmp/conglobate-rewrite-tmp passed. This is a host test-runtime setting,
  not a change to builder tmpfs storage or execution semantics.
- M3 remains incomplete: single-file recipe/stage parsing and immutable planning,
  request/CLI selection, verified Git context and local image-cache resolution
  still need implementation and their tests. No M3 adherence pass claimed.
- Added single-document stage/directive validation, distinguishing absent, empty
  and null sections; immutable pipeline order/input-role/network-step planning;
  required explicit CLI recipe/lock paths and typed context-directory selection.
- Added typed DTB values and parsed single-pass templates with dollar escaping,
  deferred undefined-variable evaluation and regex capture semantics. Shell run
  text remains shell text and is not processed by this template parser.
- Twenty behavioral tests, formatting and Clippy with warnings denied pass.
  This is a foundation slice, not completion of M3: committed private context,
  local verified cache resolution, complete COPY validation and integration
  remain. No M3 adherence pass or executor/security qualification claimed.
- Added isolated local Git object access with explicit hash recomputation for
  commits, trees and blobs; immutable captured entries retain no repository
  handle. Context selections record commit/subtree identities and object format.
  Confined file reads resolve committed symlinks against captured objects only.
- Five Git integration tests cover SHA-1/SHA-256, caller working-tree changes,
  loss of the source repository after capture, packed objects, filter avoidance,
  corrupted blob rejection, submodule/LFS rejection and confined symlink reads.
  The corruption fixture initially failed to overwrite Git's read-only loose
  object; replacing that test-owned file fixed the fixture, after which the
  intended identity-verification error was observed.
- All 25 tests and Clippy with warnings denied pass. Capture is currently an
  immutable in-memory object collection, not yet a private read-only filesystem
  checkout. Materialization, cache verification and remaining integration are
  still required for M3. No additional adherence pass claimed.
- Connected BuildRequest to CapturedBuild: recipe and lock are parsed through
  confined captured-file reads; selected names and SHA-256 of their exact bytes
  are retained for inventory. Networked plans reject an absent resolver argument;
  actual resolver capture remains setup work. Integration tests prove committed
  document selection is unaffected by caller working-tree edits.
- Added read-only cache access pinned to a directory descriptor. openat2 confines
  blob reads beneath it, rejects magic-link traversal, and nonblocking opens
  permit rejection of FIFOs before reads. Metadata is captured and hash checked;
  large blob verification streams without copying layers into scratch. Receipts
  explicitly attest only that read, not future cache contents.
- Cache tests cover modification after a successful read, wrong sizes, missing
  content, directories/FIFOs, directory rename and symlink escape rejection.
  All 30 tests and Clippy with warnings denied pass. OCI index/platform selection,
  manifest/chain validation, private materialization and remaining COPY validation
  are still M3 work; the adherence streak remains zero.
- Added host-only OCI index selection using the ecosystem VM artifact selector,
  exact index/manifest digest and size verification, incompatible/ambiguous
  platform rejection, and explicit OCI envelope checks alongside shared artifact
  validation. Captured index/manifest bytes remain available for inventory/output.
- Added streaming verification of every non-inline selected manifest descriptor.
  Metadata resolution, content-read receipts and scute-chain verification remain
  distinct: none is presented as proof of a later cache read or a verified chain.
- Seven cache tests now include direct-manifest pin rejection, ignored unrelated
  platform entries, duplicate compatible entries, size/envelope errors and missing
  referenced content. All 34 tests and Clippy with warnings denied pass.
  M3 still requires chain verification/integration, materialization and complete
  COPY validation before its three-pass gate can begin.
- COPY mode is now a validated immutable parsed value (octal or symbolic clauses),
  replacing unchecked mode strings. Multiple explicit sources require a destination
  ending in slash, enforced while parsing the recipe. Two tests cover accepted
  forms and malformed recipe/mode rejection; all 36 tests and Clippy pass.
- Reference review found the legacy symbolic evaluator is not a sufficient oracle:
  https://github.com/tonistiigi/dchapes-mode/blob/master/mode.go uses original-mode
  conditions for X and flushes partial operations around copied permission bits.
  A provisional evaluator was removed rather than carry that mismatch forward.
  The new code currently parses descriptions only. M6 must implement/reference-test
  evaluation, including mixed permission/copy expressions and X after earlier edits.
  M3 still needs filename-pattern validation, context materialization and image-chain
  verification/integration. No adherence pass claimed.
- COPY sources now contain immutable parsed SourcePattern values. Syntax and
  parent traversal are checked during recipe parsing; matching supports Unicode
  wildcard/class semantics, escaped literals and non-UTF-8 Unix candidate names.
  The grammar reference is https://go.dev/src/path/filepath/match.go. Leading and
  trailing separators retain context-relative Docker-style interpretation.
- Added source-selection tests for Unicode, classes, negation, literal braces,
  escaped metacharacters, separator boundaries, invalid UTF-8 and malformed
  patterns rejected through recipe parsing. All 38 tests and Clippy pass.
  These are syntax/name-selection tests, not COPY filesystem compatibility proof;
  wildcard walks, symlinks, ownership and mode evaluation remain execution work.
  M3 private materialization and image-chain verification/integration remain open.
- Added ScuteExtent with checked decoded size/block count/salt and canonical
  verity parameters using pinned devmap-verity. Root calculation streams bytes
  into the library formatter and discards emitted tree bytes through a seekable
  sink; it neither stages layer data nor allocates a complete hash tree.
- Exact extents reject truncation before zero padding and demand EOF afterward,
  preserving decoder trailer errors. Four tests independently check salted hash
  vectors for one/two blocks, zero padding, overflow/count validation, short/long
  inputs and late read errors. All 42 tests and Clippy pass.
- This computes scute roots only: cache stream digest checks, compressed extent
  measurement, ancestry ordering and manifest-root comparison still need to be
  connected before claiming image-chain verification. M3 remains in progress.
- Connected cache reads to scute roots and manifest ancestry verification. Every
  scute hashing pass checks the digest/length of bytes actually read; compressed
  streams are measured before verity construction and verified again while
  hashing. Layers are not copied into scratch. Canonical parameters, duplicate
  roots, missing ancestors and disconnected layers are checked; retained scutes
  are ordered base-to-top. Mount-time verification remains a separate requirement.
- Added consuming CapturedBuild::resolve, retaining cache authority and verified
  image descriptions. Execution inputs without carapaces fail. All referenced
  content is checked before privileged setup. Current preflight also performs a
  separate descriptor-verification read; eliminating that redundant scute read
  without weakening receipts is a later optimization.
- Mixed compressed/plain chain, wrong root, duplicate/disconnected layers,
  incorrect decoded extent and changed cache content tests pass. Input integration
  also checks scuteless passthrough versus command execution. The 46 tests pass
  (full suite plus new integration test), as do formatting and Clippy.
- This verifies content/ancestry, not snapshot exception structural interpretation
  or mounted behavior. Context materialization and final M3 review remain pending.
- Added private context materialization on verified existing tmpfs. A pinned
  parent descriptor anchors creation/cleanup across pathname rename; a private
  temporary directory owns all created entries and is removed on partial failure
  or Drop, with explicit fallible close also available. File modes preserve
  ordinary executable/non-executable checkout behavior under the builder umask.
- StagedContext exposes confined read-only regular-file opens and no staging path
  or writable directory descriptor. It is explicitly a staged checkout, not proof
  of a read-only mount. Read-only mount enforcement belongs to the lifecycle
  boundary and remains outstanding evidence; no command can be admitted yet.
- Two materialization tests verify committed bytes, executable bits, symlinks,
  private root mode, explicit cleanup and Drop cleanup after parent rename.
  Initial full run failed the private-root assertion: tempfile defaults permitted
  group/other access. The preceding all-pass claim was premature. Explicit 0700
  creation fixes the implementation; rerun results are recorded below.
  Next: review M3 coverage/invariants, resolve findings
  and the staged/read-only lifecycle gate before claiming clean adherence passes.
- After explicit private-root mode correction, the full 48-test suite passes,
  including rename/Drop cleanup, and Clippy passes with warnings denied. This
  correction is retained separately in history; no adherence pass was counted.
- Initial M3 context requirements review found and fixed duplicate ancestor-tree
  selection and trailing-separator symlink inconsistencies. Regression fixtures
  cover both. All 50 tests and Clippy pass. Detailed findings and the still-open
  read-only enforcement boundary are in preservation/MILESTONE-3-ADHERENCE.md.
  This is a partial self-review with findings, not a clean adherence pass.
- Safe Rust read-only bind/remount probe passed in the disposable stock Fedora
  x86-64 guest. Confirmed reopened view returns EROFS and pre-bind directory FDs
  retain writable authority, so production must replace/close those descriptors.
  Explicit unmount/cleanup also passed. Evidence, source and failed first harness
  run retained in validation/rewrite-context-mount/. Initial harness had shutdown
  log interleaving/PID1 exit; corrected v2 passes. No host mount was performed.
- Read-only lifecycle finding remains open until the owning production transition
  and failure cleanup are implemented/tested. This probe is not an adherence pass.
- Implemented consuming read-only context transition with owning mount cleanup,
  reopened directory authority, thread affinity, fallible close/retry and detach
  fallback. Staged readers now borrow their owner and cannot expose raw FDs.
- Production transition passed disposable-VM v3/v4: metadata writes return EROFS,
  held-open file causes owning EBUSY error, cleanup retry succeeds after release,
  and Drop cleanup succeeds. All 50 unit/integration tests, one compile-fail
  doctest and Clippy pass. VM evidence is preserved. Full M3 review remains open;
  no M4 command lifecycle or crash qualification is inferred.
- Pipeline review found authored transforms were not rejected during resolution
  when no output DTB could exist. Fixed preflight using only output ancestry or
  an explicit firmware-producing stage; implicit defaults retain their skip rule.
  Integration coverage includes omitted/empty/nonempty transforms without a DTB
  and an inherited-DTB success case. All 12 affected tests and Clippy pass.
  This finding keeps the M3 clean-pass count at zero; review continues.
- Cache review added matching-digest invalid-zstd cases and valid concatenated
  frames: all 11 cache tests and Clippy pass. No production change was required.
- Disposable-VM v5 validates actual partial setup: RLIMIT_NOFILE soft exhaustion
  causes reopening the remounted root to fail with EMFILE; owning error retains
  the checkout; after restoring the probe limit, cleanup retry succeeds. Existing
  normal/EBUSY/Drop checks pass too. Evidence is retained alongside earlier runs.
  This closes the ordinary setup-failure coverage gap; crash recovery remains M4.
  Evidence additions do not count as a clean frozen-scope adherence pass.
- Rust style review corrected checkout transitions to Checkout<Staged> and
  Checkout<ReadOnly>, sealed empty markers with state-specific resource payloads.
  Existing API names are aliases. All 51 tests, three compile-fail doctests and
  Clippy pass; production lifecycle/partial-setup VM regression v6 also passes.
  The style finding is resolved and recorded; clean-pass streak remains zero.
- First full-pass attempt found explicit null filesystem properties silently
  becoming an empty mapping. Regression reproduced it; optional nested mappings,
  labels and derive lists now use Presence validation consistently. All 52 tests,
  three compile-fail doctests and Clippy pass. The attempt is recorded as failed,
  not clean; the three-pass sequence must start on the corrected scope.
- First complete clean M3 pass recorded on frozen implementation scope 621b34d.
  Requirements, Rust style, safety/recovery, evidence and preservation reviewed;
  all 52 tests, three compile-fail doctests, fmt and Clippy pass. Streak is 1/3;
  M4 remains gated. This is self-review, not independent security qualification.
- Second complete clean M3 pass used failure/evidence-first traversal of unchanged
  scope 621b34d. Compile-fail diagnostics confirmed the intended ownership/state
  errors; runtime failure fixtures and their coverage boundaries were reviewed.
  Streak 2/3; M4 remains gated.
- Third complete clean M3 pass audited public authority and acceptance criteria.
  Verified preserved legacy bytes and VM probe provenance; source/tests/harness
  remain frozen at 621b34d. M3 is complete with 3/3 consecutive clean self-review
  passes. M4 may begin; no command execution or full sandbox qualification claimed.

## Milestone 4 work log

- Investigated the mount_setattr dependency gap. Existing upstream rustix proposal
  at 8bda151d exposes typed attributes with a borrowed user-namespace descriptor
  and internal structure sizing. Reviewed its libc/linux_raw implementations;
  preserved a separate pinned compile probe in validation/rewrite-mount-api.
- Both backends compile; the lifetime-negative client fails with E0597 as intended.
  Probe Clippy passes. The initial flag-name mismatch is recorded in its README.
  No production dependency changed and no host mount was performed. Actual
  idmapping/UID-GID persistence and cleanup must be tested in a disposable VM
  before adoption. This is API feasibility, not lifecycle qualification.
- Qualified the safe mount API in disposable x86-64 VMs on both linux_raw and
  libc backends. Verified offset UID/GID views, chown translation persisting after
  ext4 remount, invalid namespace rejection, failed attachment retaining detached
  ownership for retry, explicit unmount and Drop cleanup. Initial missing-loop
  fixture failure is preserved alongside passing v2/v3 evidence.
- Pinned the qualified upstream rustix revision in production. All 52 foundation
  tests, three compile-fail doctests, formatting and Clippy pass. Production
  checkout VM regression v7 also passes normal, busy/retry, Drop and partial
  setup cases. No host mounts/modules were changed. This closes the syscall API
  feasibility gap, not the M4 ownership/containment/recovery gate. Next implement
  production mount owners and namespace/identity leases using the qualified API.
- Added production Mount<Detached> -> Mount<Idmapped> with sealed state traits,
  private resource payloads, thread affinity and explicit namespace ownership.
  Failed mapping returns an owning IdmapFailure; no raw mount FD escapes.
  The detached kernel tree is released by its descriptor owner rather than a
  pathname-based destructor. Attachment and durable anchors are still pending.
- VM v4 exercises production acquisition, invalid-namespace EINVAL, ownership
  extraction/retry, mapped metadata and cleanup. It passes alongside existing
  syscall/attachment/persistence fixture checks. All 52 foundation tests and four
  compile-fail doctests pass; production and probe Clippy/formatting pass. No M4
  adherence pass claimed: command containment, LSM, attachment ownership and
  supervision/recovery remain required implementation work.
- Added Mount<Attached> and private tmpfs MountPoint ownership. A failed
  move_mount returns both owners; successful attachment performs only infallible
  ownership transfer afterward. Explicit close drops the mount descriptor before
  normal unmount and retains a recovery lease on failure. Drop detaches as a
  fallback; failed detach retains backing/namespace resources without recursive
  deletion through a live writable mount. This is not durable crash recovery or
  a filesystem sealing receipt; no commands may be admitted by these APIs alone.
- Production VM v5 passes failed attachment/retry, mapped-root metadata,
  parent-directory rename, held-open-file EBUSY cleanup retry and Drop cleanup
  preserving underlying data. Evidence and source hashes retained. All 52 tests,
  four compile-fail checks, production/probe Clippy and formatting pass. M4 remains
  incomplete with zero adherence passes; private namespace creation, resource
  anchors, LSM, process containment and storage integration remain outstanding.
