# Conglobate reimplementation goal

Status: active. No milestone may advance until its three-pass gate is clean.
The target is the latest DESIGN.md, SANDBOX-DESIGN.txt and user decisions.
This ledger describes required work, not claims about the current executor.

## Current deployment correction

The user's dedicated-VM clarification supersedes earlier guardian/recovery
requirements in this chronological ledger and in the stored goal's historical
wording. Latest user decisions remain authoritative: an independent environment
controller shuts down the VM after conglobate dies; no build resumes there.
Preserve prior code and evidence, but do not expand the guardian or serialized
cross-invocation recovery path. Ordinary cleanup, coupled policy/identity lifetime
until shutdown, command isolation and successful-publication gates remain required.
The full rewrite, distro/platform qualification and final retirement scope remain.

This changes M2's lifecycle contract: reopen its adherence gate before resuming
M4 implementation. Prior passes remain historical, not approvals of this revision.

## Milestones and acceptance criteria

| Milestone | Deliverables and success criteria | Status | Clean streak |
| --- | --- | --- | --- |
| 1. Preserve and separate | Preserve full working-tree implementation, uncommitted changes, tests and all validation sources/evidence; verify archive hashes; record Git baseline and external-artifact dependencies; keep recoverable originals before moving old code outside the new active build. | Complete | 3 |
| 2. Architecture contract | Define owning modules/types, sealed state traits, consuming transitions, fallible finalization and recovery, confined filesystem authority, typed formatter properties, inventory migration, and requirements-to-tests traceability. Identify external qualification requirements without claiming access. | Complete: revised dedicated-VM lifecycle | 3 |
| 3. Inputs and planning | New safe Rust crate; required context-relative recipe/lock paths; exact committed context and local verified image references; one recipe mapping, immutable plan, complete stage validation/inheritance; meaningful positive/negative tests. | Complete: revised context lifecycle | 3 |
| 4. Privileged lifecycle | RAM workspace, cache-backed verified layers, kernel/idmapped mounts, scoped LSM, no command SYS_ADMIN in any capability set; private namespaces, restricted descriptors/syscalls, bounded execution/setup/teardown, cancellation and descendant quiescence; partial-setup cleanup, independent VM shutdown after supervisor death and no context/control/block-device exposure. | In progress | 0 |
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
- Added thread-affine PrivateMountNamespace setup/restoration through safe nix
  APIs with fixed NEWNS only (never FILES). The rustix general safe unshare API
  is deprecated for descriptor-table concerns; no deprecated or unsafe call was
  used. Namespace propagation is made recursively private. Setup failures after
  entering retain restoration authority; Drop is only a fallback.
- Detached, mapped and attached production mount owners, including unmount
  recovery, now borrow the namespace guard. A compile-fail test prevents explicit
  restoration with a live mount owner. Single-threaded helper setup remains an
  invocation contract; the thread-count check diagnoses misuse, not racing code.
- VM v6 passes changed namespace identity, attachment invisibility from PID 1's
  root, existing lifecycle checks and explicit/Drop restoration. VM v7 adds and
  passes live-thread rejection before transition. Evidence retained. All 52 tests,
  five compile-fail checks, Clippy and formatting pass. Dedicated helper launching,
  current namespace anchors, guardian protocol and command isolation remain M4
  work; these checks do not qualify untrusted command execution.
- VM v8 validates real namespace permission failures without production fault
  hooks: the fixture temporarily removes only effective SYS_ADMIN while retaining
  permitted capabilities. Explicit restoration returns EPERM with recovery owner
  and leaves the private namespace current. Restoring the effective capability
  and retrying succeeds. Entry without the capability returns Before/EPERM and
  leaves the original namespace unchanged. Existing probe checks also pass.
  Probe Clippy/formatting pass; production code is unchanged. This qualifies the
  ordinary restoration-error path, not crash recovery or command confinement.
- Added a sealed Process<Running> -> Process<Reaped> owner for trusted helper
  children. Deadline and I/O failures retain the child owner for recovery;
  explicit termination collects status under a caller-supplied deadline. Reaped
  records this child's exit status only, never success or descendant quiescence.
  Drop attempts bounded termination/reaping and reports unresolved cleanup;
  guardian-held resource authority remains required before command execution.
- The two process tests cover successful/failed statuses and deadline ownership
  recovery followed by termination/reaping. All 54 tests, five compile-fail
  doctests, formatting and Clippy pass. These tests spawn only benign owned
  children without privileged host operations. M4 remains at zero adherence
  passes; helper launching, containment, anchors and LSM are still outstanding.
- Added independent pidfd-backed ProcessObservation acquired through the child
  owner before reaping. It exposes exit readiness only, has no public raw FD or
  signaling API, and rejects an already collected child. Unsupported syscalls or
  poll errors fail rather than falling back to numeric-PID liveness checks.
- Reviewed pidfd_open documentation and recorded its required normal SIGCHLD
  zombie semantics/exclusive reaping in ARCHITECTURE.md and API docs. The future
  trusted launcher must establish these process-wide invariants; the library
  owner alone cannot enforce them. No guardian or command admission claim made.
- New tests verify live/terminated/reaped observation, retained exit identity
  while another child runs, and rejection of a previously reaped Child. All 56
  tests, five compile-fail doctests, Clippy, formatting and diff checks pass.
  This is native Linux process evidence, not PID-reuse stress, supervisor-death,
  confidential-computing or full containment qualification. M4 remains open.
- Replaced parent-side numeric-PID observation acquisition with a trusted helper
  self-pidfd handoff. The helper pins itself while alive and transfers ownership
  through a private sequence-packet socket; the parent does not reopen a PID.
  This avoids that acquisition path's SIGCHLD/reaping assumptions. Normal child
  wait semantics and termination ownership still need launcher integration.
- The one-shot protocol uses deterministic CBOR [1, 0] plus exactly one
  SCM_RIGHTS descriptor, bounded nonblocking waits, close-on-exec ownership and
  strict framing/descriptor counts. Startup replaces inherited socket stdin with
  /dev/null to avoid leaving a second descriptor available to later execs. Safe
  rustix APIs cover every syscall. Initial socket-option API spelling error was
  corrected before testing. No nightly API or application unsafe was introduced.
- Native tests exercise a fresh child transferring its own identity, observation
  after exit/reaping, live observation, CLOEXEC, stdin replacement, peer loss in
  either direction, a silent-peer deadline, malformed/truncated CBOR and missing,
  extra or truncated descriptors. This is identity exchange only: no readiness,
  admission acknowledgement, guardian recovery or command containment receipt.
  M4 remains at zero adherence passes. The launcher must now integrate this
  exchange with stable termination ownership and the registration/admission gate.
- Full suite passes: 60 tests (including the subprocess fixture entry) and five
  compile-fail doctests. Clippy, formatting and diff checks pass. Existing
  validation artifacts and legacy reference code remain preserved unchanged.
- Connected trusted launch to the identity exchange. Process<Identified> has a
  distinct sealed state payload whose termination authority owns the helper's
  pidfd. Explicit termination and Drop use that identity without numeric-PID
  fallback; wait errors retain the same state and ownership. Process<Running>
  remains the pre-handoff wait-owned child with its documented reaping contract.
- Launch errors distinguish no child from a started child requiring recovery.
  The parent drops Command's retained child socket endpoint before receiving,
  ensuring EOF reflects helper loss. Trusted executable/configuration and normal
  SIGCHLD/exclusive-reaping prerequisites remain caller obligations. No recipe
  execution, containment registration or admission permission is implemented.
- Tests cover identified success, wait timeout/retry and termination, missing
  executable, expired pre-launch deadline, early peer exit, silent-peer timeout,
  and Drop termination/reaping observed independently through pidfd HUP. A
  compile-fail test prevents asserting Identified from an arbitrary Child.
  Full suite passes 64 tests (including fixture entry) and six compile-fail
  doctests; Clippy, formatting and diff checks pass. M4 remains at zero adherence
  passes; guardian, admission gates, anchors, LSM and containment are incomplete.
- Added the rewrite's scoped label policy asset, adapted from the preserved
  kernel-mount prototype. It separates the command namespace/mapped root subject
  from an explicit set of superblock owner/device targets. Two entries support
  bootstrap's tool and output roots; no owner-only wildcard is accepted. Hooks
  retain exact attribute-name checks and prior LSM results. Program-side map
  writes are prohibited. Runtime map/link lifecycle is not yet integrated.
- Disposable no-NIC/no-disk/no-share x86-64 VM runs pass with one target and two
  targets. The former denies labels on a second same-owner ext4 mount; the latter
  permits label set/readback/remove there. Existing exact-name, mapped-nonroot,
  foreign-namespace, unassigned tmpfs, offline Fedora installation, fsck and
  independent remount metadata checks also pass. Loader-death pin retention is
  fixture evidence only. Both runs and source/input/provenance hashes are retained
  under validation/rewrite-label-policy; all prior validation remains untouched.
- Added immutable Rust LabelScope/FilesystemTarget descriptions and safe native
  map encodings. Reject invalid/truncated namespace or device identities,
  zero/sentinel root UID, duplicate/empty/oversized target sets and command-owned
  superblocks. These numeric descriptions confer no authority and do not pin
  identities. Activation will require the still-missing anchored lifecycle owner.
- Full Rust suite passes 67 tests (including fixture entry) and six compile-fail
  doctests; Clippy and formatting pass. Policy C compiles with warnings as errors.
  No host policy or mount was changed. Safe Rust loader integration, partial
  installation/crash recovery, concurrency, Arm and confidential qualification
  remain required; M4 still has zero adherence passes.
- Integrated libbpf-rs/libbpf-cargo 0.27.2 with locked libbpf-sys 1.7.0. The build
  compiles and embeds the trusted policy source without generating a Rust
  skeleton. Reviewed safe object/map ownership APIs and BSD license alternatives;
  documented clang/C/libelf/zlib prerequisites. No host package installation or
  policy load was performed, and application/build-script unsafe remains forbidden.
- Added sealed LabelPolicy<Prepared> -> LabelPolicy<LoadedDisabled>. Preparation
  checks the embedded maps' types, sizes/capacities and all three hook sections.
  Loading verifies zero subject config and writes the validated target entries.
  No attachment, pinning or activation API exists yet. Errors release unattached
  resources rather than leaving enabled authority or a misleading success token.
- Disposable-VM loader v1 passes repeated production load/drop with descriptor
  count returning to baseline. v2 additionally clears the probe's effective
  capabilities, observes load failure without descriptor leakage, restores
  capabilities and successfully reloads/drops. Both evidence sets are preserved;
  v2 records final source hashes. These are process-FD and disabled-load checks,
  not global leak, active-hook, crash-recovery or command-confinement proofs.
- All 68 Rust tests (including fixture entry), six compile-fail doctests,
  production/probe Clippy and formatting pass. M4 remains at zero adherence
  passes. Next policy work must establish anchor ownership and disabled pinned
  guards before any activation/admission API can be justified.
- Added LoadedDisabled -> PinnedDisabled ownership: a protected root-owned bpffs
  parent supplies a private 0700 directory; descriptor-relative pin paths retain
  directory authority. Both maps are pinned before set/remove guard attachment,
  with skipcap attached/pinned last. Config remains zero and no activation API
  exists. Reviewed libbpf link destruction: LSM link ownership closes link FDs;
  pin paths are separately removed by the disabled resource owner.
- Setup errors return partial disabled resources; cleanup errors retain them
  for retry. Explicit close and Drop remove only the known pins and an empty
  directory, never recursively deleting unknown contents. This strategy is valid
  only for disabled policy, not enabled authorization. Directory descriptors do
  not establish the independent namespace/mount anchors required for activation.
- VM v3 initially failed descriptor accounting because the probe retained a
  Vec<DirEntry> witness. Preserved that failure and source. v4 drops the witness
  before accounting and passes normal close, Drop, and a check_remove directory
  collision after maps/check_set were pinned. The collision also causes cleanup
  failure; removing it and retrying through the retained recovery owner succeeds.
  Production code did not change between v3/v4. Source/provenance evidence retained.
- Rust checks cover rejection of ordinary filesystems as pin parents; all 69
  tests (including fixture entry), six compile-fail doctests and production/probe
  Clippy/formatting pass. No host BPF attachment or mount occurred. M4 remains at
  zero adherence passes. Independent recovery anchors, enabled-policy lifetime,
  guardian admission and command containment are still required implementation.
- Added kernel-held NamespacePin bind mounts with immutable identity records.
  Dropping the userspace pin owner leaves the mount for trusted recovery. Reopen
  verifies nsfs inode/device without following symlinks; release errors retain
  either the mounted pin or its unmounted target. Creation errors after target
  allocation retain that target for cleanup. No raw namespace FD escapes.
- VM v5 demonstrates reopening after userspace-owner drop and namespace-creator
  exit, then permission-denied unmount recovery. v6 adds denied creation cleanup
  and substituted-symlink rejection. Review found the parent must be root-only
  because mounted nsfs files can be readable; v7 requires mode 0700, rejects the
  non-private fixture parent and passes the lifecycle. All versions are retained.
  These tests do not verify production independent-mount-namespace selection,
  persist a build lease, or establish full supervisor/guardian crash recovery.
- The broader test run exposed peer_loss_is_an_error_in_both_directions racing
  with temporary inherited CLOEXEC descriptors during another trusted spawn.
  IdentityReceiver Drop now shuts down its endpoint instead of depending solely
  on descriptor closure. A duplicate-endpoint regression tests this deterministically.
  This establishes normal-drop cancellation, not parent-death recovery or an ACK.
- Full suite passes 71 tests (including fixture entry) and six compile-fail
  doctests. Final permission refinement also passes VM v7 and targeted anchor
  tests; production/probe Clippy and formatting pass. M4 remains at zero clean
  adherence passes. The next integration must establish a trusted independent
  anchor namespace, persist/lock the lease, and combine namespace/filesystem/policy
  lifetimes with containment before exposing activation or admission.
- Namespace pin creation/recovery now require RecoveryDirectory, a protected
  tmpfs directory opened without following its final symlink and locked with
  nonblocking exclusive flock. Pins and unmounted targets internally share the
  open description; the lock closes only after the last resource owner releases
  it. No public Clone, raw descriptor or early-unlock operation is exposed.
- VM v8 passes competing-open exclusion and lock retention after facade Drop.
  v9 extends that to failed-creation targets and recovery after directory rename.
  v10 executes a separate probe process, which fails acquisition while held and
  succeeds after release. Existing namespace/policy lifecycle checks still pass.
  All evidence and source hashes retained. This remains cooperative ownership;
  privileged protocol bypass and containing-directory replacement are outside
  its guarantee. Persisted records and independent anchor-namespace checks remain.
- All 71 tests (including fixture entry), six compile-fail doctests, production/
  probe Clippy and formatting pass. M4 has zero adherence passes and remains open;
  no activation, guardian admission or full crash-recovery claim is made.

- Namespace recovery now has a versioned deterministic CBOR description and
  lease-scoped save/load/remove methods. Parsing is bounded and canonical and
  creates data only. Publication verifies the live pin, uses a private staged
  file and no-replacement publication, and documents post-publication errors.
  Deletion requires matching stored identity and absence of the mount target.
- Added minicbor 2.3.1 (BlueOak-1.0.0, permissive) using its safe encoder/decoder
  APIs with std and no derive feature. Application Rust still forbids unsafe.
  The namespace control record is separate from the output inventory schema.
- Codec tests cover exact bytes, truncation, canonical form, bounds and invalid
  names/identities. VM v11 passes stored-record recovery and publication/removal
  ordering; v12 adds record-file rejection cases. An initial v12 probe compile
  required importing PermissionsExt; this did not affect production code or v11.
  Full Rust tests and six compile-fail doctests pass; production/probe Clippy
  pass. M4 remains open with zero adherence passes. These records are not yet a
  complete persisted build lease or proof of independent namespace lifetime.

- RecoveryDirectory now retains a deployment-init mount namespace descriptor and
  pidfd in the shared directory lease. Acquisition rejects PID 1, mismatched procfs
  PID views, different PID namespaces and a private current mount namespace.
  Creation, verification/release and record operations recheck namespace identity
  and init liveness. Failed release retains the pin owner. These are methods on
  immutable private ownership types; no new raw-FD or signaling API is exposed.
- This verifies deployment-relative namespace identity, not host-global identity
  or future init behavior. The deployment must keep init independently alive and
  in its namespace. A complete build lease, guardian and admission protocol are
  still required; the new check alone cannot authorize commands or activation.
- VM v13 fails an older mount-denial fixture because clearing every effective
  capability now denies the earlier procfs check. v14 targets SYS_ADMIN removal
  for mount tests while leaving the BPF all-capabilities-denied test intact.
  v14 passes carried-lease rejection in a private namespace, ownership recovery
  after restoration, and recovery after a separate publisher exits without Rust
  destructors. Failed and passing runs, source snapshots and hashes are retained.
- Full Rust suite and six compile-fail tests passed; after the final PID-view
  recheck refinement, affected anchor/codec tests and VM v14 passed. Root/probe
  Clippy and formatting passed. M4 remains at zero adherence passes; neither
  these tests nor the primitive constitute full sandbox qualification.

- Added sealed Supervision<Unpaired/Paired> startup states with mutual self-pidfd
  handoff over a retained private CLOEXEC sequence-packet channel. Reused the
  existing bounded strict identity codec without exposing raw descriptors.
  ControlSocket RAII shutdown signals normal cancellation; peer pidfd observation
  also detects exit without destructors. A bounded poll waits on both sources.
- Added launch_supervised on the identified process owner. Before-spawn failures
  own no child; exchange/duplication failures cancel the channel and return the
  pending child. Success retains independent pidfd references for supervision
  and child termination/reaping. No guardian readiness or admission is implied.
- Maintained child-process tests cover live peer and wait deadline, forced exit,
  exit bypassing destructors, normal channel cancellation, silent exchange
  timeout, supervised launch identity and retained ownership on exchange failure.
  A compile-fail test rejects observation before pairing. Initial compilation
  corrected the rustix recv tuple return type; no unsafe interface was added.
- Final full suite passes 81 tests (including fixture entrypoints) and seven
  compile-fail doctests. Clippy and formatting pass. M4 remains open with zero
  adherence passes. Persisted aggregate ownership, actual guardian/containment,
  readiness/registration/acknowledgement and combined death recovery are pending.

- Pre-integration review found supervision consumed unexpected traffic before
  returning InvalidData. A caller making another observation could then see a
  healthy channel. Changed observation to MSG_PEEK, preserving the failure without
  a mutable error latch. A regression checks repeated observations and bounded
  waiting all reject the same queued invalid payload.
- Added separate mechanism tests: normal cancellation remains visible while a
  duplicate endpoint is held, and pidfd-observed process exit remains visible
  while the peer socket is deliberately kept open. The latter uses an exclusively
  owned child PID solely to construct the test fixture; production startup still
  exchanges self-pidfds. These tests do not establish descendant quiescence.
- Three new unit tests pass, alongside the existing identity/supervision
  integration checks, Clippy and formatting. This is an in-progress M4 correction,
  not an adherence-gate pass. The clean streak remains zero.

- Added CommandDomain for fresh private cgroup2 creation and explicit subtree
  cancellation. Protected-parent validation rejects ordinary filesystems before
  mutation. Bounded event parsing rejects missing/ambiguous population status.
  Setup errors after creation retain a target; cancellation/removal errors retain
  the domain. Drop leaves kernel ownership for recovery, never reports completion.
- The primitive creates no limits/controllers and exposes no arbitrary-existing-
  cgroup kill or numeric-PID registration API. Production admission must still
  prevent forks before registration, keep control processes outside membership,
  persist domain identity and prevent access/migration by commands. Empty-group
  removal is not reaping, policy cleanup or a successful-step receipt.
- Disposable VM v15 passes empty cleanup and shell-plus-child cancellation after
  a fixture-only pre-fork gate. v16 also forces parent-removal failure with a
  nested empty cgroup, recovers the returned owner and later cancels successfully.
  Both retain prior namespace/policy checks; sources, hashes and logs preserved.
- Full suite passes 86 tests and seven compile-fail doctests. Root/probe Clippy
  and formatting pass. All cgroup mutation occurred in disposable QEMU guests.
  M4 remains at zero adherence passes, with production registration, persisted
  domain recovery and complete guardian integration still pending.

- Added Supervision<Registered> and explicit parent/helper registration exchange.
  Parent passes an owned cgroup.procs descriptor; trusted single-threaded helper
  writes its own membership, closes the descriptor, then acknowledges. Parent
  performs no numeric-PID lookup. Failures cancel the channel and leave domain
  and child owners with callers. Registered is an acknowledgement, not admission.
- Canonical bounded registration packets require the exact version/opcode and
  descriptor count. Received descriptors are CLOEXEC. Tests reject malformed
  framing, wrong opcodes/versions, missing/extra descriptors, peer loss and timeout;
  successful offer/ack transfers preserve ownership. An initial test compile
  corrected fcntl_getfd/FdFlags from rustix::fs to rustix::io.
- VM v17 passes production self-registration, descriptor closure and inherited
  child membership followed by subtree cancellation. v18 adds missing-ACK
  rejection with retained domain cleanup and explicit child reaping. Prior
  namespace/policy/domain-removal checks remain. Sources and evidence preserved.
- Full suite passes 89 tests plus seven compile-fail doctests; root/probe Clippy
  and formatting pass. M4 remains at zero adherence passes. Persisted domain
  records, aggregate recovery ownership, guardian orchestration and final
  sandbox admission still need implementation and combined fault validation.

- Domain persistence preparation found live command domains did not retain the
  recovery lock. Creation now requires RecoveryDirectory and internally retains
  its Arc-backed lease in DomainTarget, covering both complete domains and setup
  failures. Registration, cancellation and target removal verify the retained
  deployment namespace. No public clone/raw-FD/unlock interface was introduced.
- VM v19 passes facade-drop exclusion for a live domain, independent-process
  exclusion, retained ownership after failed removal and successful lock release
  after final cleanup. v20 adds wrong-mount-namespace cancellation rejection on
  an otherwise removable domain, followed by restoration and successful use.
  Evidence and exact probe/source snapshots are preserved.
- Affected domain tests, root/probe Clippy and formatting pass. This prerequisite
  is complete; domain record serialization, identity-checked reopening and the
  aggregate guardian protocol are still pending. M4 remains at zero adherence
  passes; this is not a persistence or crash-recovery qualification claim.

- Added canonical bounded DomainRecord with generated name plus parent/domain
  device/inode identities. Decoding grants data only. RecoveryDirectory saves
  only a live domain owned by that same lease, loads protected records and
  reopens only after parent/cgroup identity, permissions and type verification.
  Record deletion requires an exact stored match and absent target in its
  original parent. No arbitrary decoded description becomes a domain owner.
- Namespace/domain persistence now shares ControlRecords for bounded private
  no-replacement publication and file validation. Post-publication errors can
  leave a complete record and must fail the build; no silent overwrite occurs.
- Three new codec tests cover exact bytes, bad names/IDs, truncation, oversized,
  trailing, unknown-version and noncanonical records. VM v21 passes populated
  domain recovery after owner Drop and wrong-parent/premature-deletion rejection.
  v22 adds wrong-lease save rejection, replacement-directory rejection and an
  independent publisher exiting without destructors. All evidence preserved.
- Full suite passes 92 tests and seven compile-fail doctests. Root/probe Clippy
  and formatting pass. M4 remains at zero adherence passes. Complete aggregate
  persistence, exclusive recovery arbitration, guardian orchestration and final
  admission still require implementation and combined fault qualification.

- Connected persistence to registration with DomainRegistration, a private-field
  borrowed capability. CommandDomain::registration compares the current live
  identity to the protected record through its retained lease; descriptor handoff
  checks again. Bare domains no longer satisfy register_helper_until. The borrow
  prevents consuming the domain during exchange and introduces no runtime flags.
- A compile-fail test rejects bypassing the capability. VM v23 rejects an unsaved
  domain and a record corrupted after capability creation; the helper receives
  channel closure before registration/child creation. Canonical restoration
  permits the ordinary path. Missing-ACK cleanup now includes its persisted
  record, keeping that negative test beyond the new publication prerequisite.
- Full suite passes 92 tests and eight compile-fail doctests. Root/probe Clippy
  and formatting pass. The domain-record gate is not complete build persistence
  or sandbox admission. M4 remains open with zero adherence passes; aggregate
  ownership and actual guardian orchestration remain required.

- Added GuardianWatch borrowing paired supervisor and registered helper channels.
  It waits on both sockets and pidfds under one bound and returns sampled loss
  diagnostics only. Errors propagate for failed-build cancellation; no observation
  becomes a quiescence or completion receipt. Setup-phase monitoring is separate.
- Unit checks cover supervisor/helper/both loss, deadline and invalid traffic.
  A compile-fail check prevents constructing this registered-phase watch around
  an unregistered helper. VM v24 uses a separate supervisor-peer stand-in, observes
  its termination while the helper remains live, then reopens/cancels the domain,
  reaps the helper and observes both losses. Exact evidence/source is preserved.
- Full suite passes 93 tests and nine compile-fail doctests. Root/probe Clippy
  and formatting pass. M4 remains at zero adherence passes. The borrowing watch
  is not the owning guardian aggregate; startup monitoring, resource teardown
  orchestration and combined crash qualification remain unfinished.

- Added RecordedDomain owning a cgroup and its protected record under a borrowed
  recovery lease. Cancellation consumes it; success yields DomainRecordCleanup,
  and record removal consumes that owner. Failures retain resource authority.
  A publication failure retains the raw domain because record publication may
  already have occurred. Post-unlink errors are never inferred to be success.
- Review of the initial v25 aggregate found cancellation errors returned an owner
  capable of registration. Tightened the state boundary: failures and recovery
  yield DomainCancellation with cleanup retry only. v26 first completes failed
  cancellation and creates a new domain for registration, then exercises recovery,
  populated cancellation and failed record cleanup/retry. Both runs are preserved;
  v26 qualifies the final contract. Compile-fail tests enforce both phase barriers.
- An initial fixture compile rejected borrowed cleanup errors escaping as static
  boxed errors. Expected error owners are retained/retried; terminal unexpected
  fixture errors become diagnostics before disposable-guest exit, not production
  completion. Full suite passes 93 tests and eleven compile-fail doctests; root/
  probe Clippy and formatting pass. M4 remains at zero adherence passes. The
  owning helper/guardian, complete resource ordering and combined death tests
  remain unfinished; this aggregate covers domain/record ownership only.

- Added ContainedHelper, constructed only by RecordedDomain::launch through the
  persisted registration gate. It owns the identified helper, registered channel
  and recorded domain. Consuming cancellation closes the channel before domain
  kill/removal; HelperReaping must collect helper status before ReapedHelper can
  remove the record. Failed transitions retain cleanup-only resource owners.
- VM v27 passes cancellation failure/retry, record corruption failure/retry,
  missing registration ACK, pre-spawn failure and started-child handshake timeout.
  The fixture explicitly reaps retained startup children and cleans their domains.
  Evidence includes exact source snapshots and binary/kernel/initramfs hashes.
  A compile-fail example forbids finishing record cleanup before helper reaping.
- Full suite passes 93 tests and twelve compile-fail doctests; root/probe Clippy
  pass. Review checked private association, consuming error ownership and channel
  close ordering. No runtime forced helper-reaping failure was exercised. This
  component is not recipe admission, policy/mount teardown or all-descendant
  reaping. M4 remains open at zero adherence passes; full guardian ownership and
  combined crash qualification remain required.

- Added RegisteredGuardian owning upstream supervision and ContainedHelper. Its
  consuming supervise_until attempts cancellation for every observation outcome,
  including timeout/errors. GuardianStop keeps observation diagnostics distinct
  from cleanup success/failure and is must-use. Callers retain only reaping or
  cleanup-retry owners, never registered execution authority.
- VM v28 passes supervisor loss and timeout with both peers alive through this
  owning path, followed by helper reaping and protected-record removal. Review
  corrected an overclaim that cancellation always sends cgroup.kill: namespace,
  identity and I/O checks can fail before the write. v29 additionally exercises
  cancellation failure after supervisor loss and cleanup-owner retry.
- Root/probe Clippy pass; the full suite passes 93 tests and twelve compile-fail
  doctests. These component checks do not qualify the independent guardian
  process, setup monitoring, adopted-descendant reaping or policy/mount teardown.
  M4 remains open at zero adherence passes. Previous goal turn was progress:
  committed contained-helper ownership with disposable-VM evidence.

- Added ChildReaper for the dedicated guardian's adopted descendants, using safe
  nix prctl/wait interfaces. Installation rejects init, multiple threads, existing
  children and an already-enabled setting. Process-lifetime adoption is not undone
  by Drop. The owner is thread-affine; bounded consuming drain retains its count
  and owner on failure and uses __WALL to include Linux clone children.
- VM v30 passes adoption after parent exit, timeout retaining the owner, retry
  collecting one orphan and empty child-list verification. v31 additionally
  rejects installation while a direct child exists, then succeeds after that
  child's explicit termination/reaping. Review tightened task enumeration to
  propagate directory-entry errors rather than merely counting them.
- Full suite passes 93 tests and twelve compile-fail doctests; root/probe Clippy
  pass. Clone-child behavior is implemented via __WALL but not separately exercised
  here. The drain does not prove domain membership, sandbox quiescence or policy
  cleanup. Guardian integration must close admission and reap individually owned
  children before invoking this process-wide wait. M4 remains at zero adherence
  passes. The previous goal turn was progress through committed guardian
  cancellation ownership and VM qualification of its failure paths.

- Connected ReapedHelper and ChildReaper through consuming DescendantDrain and
  DrainedHelper owners. Failed drain retains the helper record and reaper/count;
  failed record cleanup retains the successful drain result. A compile-fail test
  forbids record cleanup on the draining state. Low-level record-only cleanup
  remains distinct and cannot establish complete step cleanup.
- VM v32 creates a registered helper that leaves a benign sleep orphan in its
  domain. Production cancellation removes the domain, helper wait precedes the
  process-wide drain, an expired drain deadline retains both owners, retry reaps
  exactly one adopted child, and corrupted-record cleanup retains/retries the
  drained result. It verifies no children and no recovery files remain.
- Full suite passes 93 tests and thirteen compile-fail doctests; root/probe Clippy
  and formatting pass. Review checked failure ownership and ordering; dedicated
  guardian startup still must guarantee subreaper installation precedes spawning
  and prohibit unrelated wait owners. Policy/mount integration remains unfinished.
  M4 stays at zero adherence passes. Previous goal turn was progress: committed
  descendant-reaping primitive and guest-only adoption/failure evidence.

- Added an independent-process guardian fixture composing production lifecycle
  owners across supervisor, guardian and registered helper processes. v33 passes
  upstream channel closure and supervisor exit without Rust destructors. The
  guardian cancels, reaps, drains and removes its record; lease acquisition is
  rejected while it owns the build and succeeds after cleanup.
- v34 adds guardian termination through its identified owner, peer-loss detection,
  supervisor recovery under the acquired lease, persisted-domain cancellation,
  adopted-child draining and final record removal. Review found a fixture file
  readiness race; v34 publishes the completed readiness file through rename.
  Exact source and binary/kernel/initramfs evidence is retained for both versions.
- Only the validation harness and documentation changed; probe build, Clippy and
  formatting pass. Root tests were not repeated without production changes. This
  is registered-phase domain qualification across real processes, not complete
  sandbox qualification. Production startup/admission, setup monitoring, policy/
  mount integration and concurrent death cases remain outstanding. M4 is still
  at zero adherence passes. The previous goal turn made progress by committing
  integrated descendant drain and ownership-preserving cleanup retries.

- Added private ExchangeWait with deadline-only and StartupWatch implementations.
  Watched launch/registration poll the helper socket and upstream socket/pidfd;
  upstream loss or invalid traffic aborts exchanges without discarding child wait
  ownership. Existing standalone exchange behavior is retained through the same
  packet codecs. StartupWatch checks loss before and after poll, including when
  helper readiness is simultaneous. Kernel spawn itself remains uninterruptible.
- Unit coverage verifies stalled exchange interruption, rejection before spawning
  when upstream is already lost and sticky malformed traffic. VM v35 exercises
  actual upstream process termination during helper identity and registration-ACK
  stalls, followed by explicit child and persisted-domain cleanup. v36 adds a
  two-second response bound against the five-second setup deadline. An initial
  probe compile caught reversed record-removal arguments; corrected before runs.
- Full suite passes 94 tests and thirteen compile-fail doctests; the tightened
  unit check, root/probe Clippy and formatting also pass. Evidence/source hashes
  are preserved. The owning startup aggregate still must select watched methods;
  other setup operations and final admission remain unimplemented. M4 remains
  open at zero adherence passes. Previous goal turn made progress through
  independent-process domain lifecycle and guardian-death recovery evidence.

- Connected watched identity and registration to RecordedDomain::launch_watched.
  Both standalone and watched paths share one consuming finish_launch handler:
  launch errors retain any started child and cleanup-only domain; registration
  errors retain the identified child and cleanup-only domain. No mutable phase
  flags or duplicate failure cleanup logic were introduced.
- VM v37 interrupts both owned startup phases through upstream process death,
  checks the expected retained child type and protected record, and explicitly
  terminates/reaps and cancels/removes the domain. A two-second response bound
  remains against a five-second setup deadline. The independent guardian fixture
  now uses launch_watched, exercising successful registration and subsequent
  supervisor/guardian loss recovery through this owned path.
- Full suite passes 94 tests and thirteen compile-fail doctests; root/probe Clippy
  and formatting pass. Review checked shared failure ownership, callback borrows
  and phase evidence. The fixture's scheduled termination is not race-completeness
  proof. Non-exchange setup monitoring, full guardian startup/admission and
  policy/mount integration remain unfinished. M4 stays at zero adherence passes.
  Previous goal turn was progress through watched-exchange implementation and
  measured disposable-guest interruption evidence.

## Dedicated-VM revision review

- Previous user-facing explanation alone was not implementation progress. This
  turn changed the authoritative lifecycle contract to match the user's deployment
  clarification and preserved the superseded documents with SHA256 checksums.
- A review found section 12.3 still required parent-death recovery and same-VM
  concurrent builds. Replaced those with helper-death cleanup, supervisor-death
  VM shutdown and independent build VMs. Clarified interrupted activation as
  in-process cleanup rather than resuming an abandoned VM. Finding resets the
  revised M2 streak to zero; M4 implementation is held pending three clean passes.
- No production code or historical experiments were deleted. The uncommitted
  command-integration harness and its successful/failed guest evidence remain
  intact. Its diagnostic build completed before this design revision; it has not
  been mistaken for a passing sandbox qualification.

- Revised M2 completed three consecutive clean self-reviews; scope hashes and
  category-by-category observations are in
  preservation/MILESTONE-2-VM-REVISION-ADHERENCE.md. M4 resumes at zero passes.
  No production qualification is implied. Prior explanation-only turn was no
  progress; this turn verified preservation and completed the architecture gate.


## Integrated command fixture

- Combined existing mount, idmap, domain, helper and reaping owners with a shell
  in an ext4 root inside disposable QEMU. Sources and v1-v5 evidence are preserved
  in validation/rewrite-command-integration; no production source changed.
- v3 diagnosed the prior startup failure: Bubblewrap left a nonzero capability
  bounding set. A trusted fixture setpriv finalizer removes it before shell exec.
  v4 passed command/timeout/remount checks but failed due to missing fixture rmdir;
  v5 includes it and passes. Failed runs remain preserved. Clippy and fmt pass.
- This evidence covers actual image writes, all five empty capability sets,
  no_new_privs, UID mapping, timeout cancellation, descendant reaping, clean
  unmount/e2fsck/remount. It does not qualify the final sandbox. No active labeling
  policy or seccomp, final build capability profile, trusted launch root handling,
  bootstrap, or supervisor-death VM disposal is covered. Empty timeout mount-point
  cleanup is still fixture-owned. M4 remains in progress, zero adherence passes.
- Next integration work must replace fixture launch/path discovery with trusted
  executable and scoped filesystem authority, retaining normal-error ownership;
  then integrate policy admission and actual environment shutdown qualification.


- Previous goal turn made progress through the lifecycle design commit and
  integrated command evidence. This turn reviewed trusted launch authority:
  fixture setpriv is image-resident and must not become the production trusted
  finalizer. Production must retain trusted code/dependency authority until
  privileges are finalized, then execute the image command. No public raw-root
  descriptor escape or generated-path discovery API was added to bypass this.
- Command fixture v6 proves its independent controller can observe supervisor
  SIGKILL while the command cgroup is still populated, then power off QEMU with
  no command-success marker. No guardian or resumed build participates. v7 passes
  normal completion/timeout/remount regressions with the updated source. Both
  runs, init scripts, logs and hashes are preserved; Clippy and formatting pass.
- This is evidence for the dedicated-VM lifetime arrangement, not production
  appliance qualification or publication rejection. Active-policy lifetime
  across the shutdown interval remains untested. M4 remains at zero passes;
  production trusted launch and policy admission are still the next integration
  work, rather than further guardian/recovery protocol development.


- Previous goal turn made progress through supervisor-death VM disposal evidence.
  Added native CommandPrivileges in the production process module using safe
  rustix APIs (thread feature). Immutable candidate profile is consumed by apply;
  no free utility functions, runtime state flags or unsafe application code.
  Irreversible failure explicitly requires helper exit, not rollback or retry.
- It probes kernel-supported v3 capability bits, removes disallowed bounding bits
  while SETPCAP remains effective, empties ambient/inheritable sets, sets
  no_new_privs, applies/verifies permitted/effective sets and verifies bounding
  and ambient sets. SYS_ADMIN is excluded. The candidate filesystem allowlist
  still needs actual distro/package and syscall/LSM integration qualification.
- v8 passed post-exec native privilege checks. v9 preserved a negative-fixture
  ordering failure: user namespace creation restored the capability removed
  before creation. v10 performs the removal inside the namespace and rejects
  execution; positive user-namespace checks and existing image/timeout/remount
  regressions pass. Native code snapshots and failed evidence are preserved.
- Root cargo test passes, including thirteen compile-fail doctests; root and
  fixture Clippy with warnings denied and formatting pass. Review covered
  irreversible partial failure, unknown capability bits, private policy fields,
  absence of host-side privilege mutation and evidence limits. M4 remains open
  with zero adherence passes: the complete trusted launch path still uses test
  adapters, and active policy/syscall admission has not been integrated.


- Previous goal turn made production progress on native capability finalization.
  Added immutable CommandSyscalls compiled through safe seccompiler 0.5.0
  (Apache-2.0 OR BSD-3-Clause), using linux-raw-sys ABI constants. No application
  unsafe code or freestanding helper functions. Consuming installation documents
  partial-filter failure as exit-only; it is not a successful admission receipt.
- Default-deny profile preserves ordinary build syscalls, limits clone namespace
  flags, supplies clone3 ENOSYS fallback, and constrains socket/prctl/ioctl args.
  Review checked default action, filter installation order, architecture guard,
  native syscall numbers and continued exclusion of mount/namespace/keyring/BPF
  and process-inspection interfaces. Full syscall and ABI qualification remains.
- v11 found FIONBIO missing from the inherited prototype ioctl list: local Rust
  std uses it for Command::output pipe handling. Preserved the failure and added
  only this descriptor-local operation. v12 passes capability finalization plus
  filter installation, thread creation, ordinary file I/O, subprocess capture,
  denied user-namespace creation and post-exec capability checks. Existing image
  command/timeout/remount regressions also pass. Both runs have source snapshots.
- Root tests including thirteen compile-fail doctests pass. Root/fixture Clippy
  and formatting checks are recorded separately; M4 remains at zero passes.
  These native restrictions still need integration into image-root launch and
  LSM admission. Candidate package compatibility, individual denied interfaces,
  arm64 runtime and confidential-computing qualification remain outstanding.


- Previous goal turn made production progress on native syscall restrictions.
  Added consuming CommandRoot -> EnteredRoot with a borrowed private mount
  namespace, thread affinity, descriptor-based clone/attach/pivot/old-root detach,
  and an exec-only native restriction path. Trusted setup code is already loaded
  before pivot; no image-resident finalizer or library is trusted for setup.
- Root setup failures v13-v16 are preserved. Diagnostics identified old mount
  descriptors across namespace unshare and cwd pointing beneath a self bind.
  Open root after entering its borrowed namespace, clone/attach via mount FDs,
  and fchdir to the new tree. v17 passes; v18 removes image setpriv, asserts its
  absence, and passes actual image shell, capability profile, timeout, image UID
  persistence and clean fsck/remount checks. No production root path is exposed.
- Exec checks real procfs and close-on-exec flags on nonstandard FDs, clears env,
  supplies fixed PATH/HOME/LANG and null stdin, then applies native capabilities
  and syscall restrictions. Errors are exit-only, not rollback or success. A
  compile-fail test prevents restoring the namespace with a live selected root.
- Root tests including fourteen compile-fail doctests, root/fixture Clippy and
  formatting pass. Reviewed lifecycle borrow, owned descriptor closure, private
  mount propagation, pivot failure handling and claim limits. This is not a
  milestone adherence pass: M4 remains zero. External namespace/proc adapters,
  runtime view hardening, FD negatives, init reaping, active policy admission,
  complete normal-error ownership and native-path VM disposal still need work.


- Previous goal turn made progress integrating native image root/restrictions.
  Native RuntimeTargets now resolves proc/sys under the pinned image root via
  openat2, forbids magic links/mount crossings and rejects root, duplicate and
  nested target aliases before mounting. Four nonprivileged maintained tests
  pass. No generic mount authority or raw descriptors are exposed publicly.
- Replaced the fixture's external proc mount with safe fsopen/fsconfig/fsmount/
  move_mount construction of fresh procfs and empty read-only sys tmpfs. Detached
  FDs use RAII; partial attached setup remains exit-only under namespace ownership.
  Existing runtime directories are currently required; scaffolding cleanup and
  private dev/devpts/shm plus sensitive proc masks remain required work.
- v19 passes native setup, read-only sys, command/timeout/capability/ownership and
  clean filesystem persistence tests. v20 shows correct native-path supervisor
  death and VM power-off but failed serial parsing due to interleaved printk.
  Preserved its failing verdict; corrected userspace-marker parsing without
  removing the requirement for a raw kernel power-down message. Directed parser
  success/failure cases pass, and fresh v21 independently passes VM disposal.
- Root tests, including four new confinement cases and fourteen compile-fail
  doctests, root/fixture Clippy and formatting pass. Reviewed target resolution,
  overlap rejection, mount flags, partial setup ownership and evidence limits.
  M4 remains at zero adherence passes. No active policy or full launcher/distro/
  platform qualification is implied; namespace adapters and remaining runtime
  views still require implementation and integration.


- Previous goal turn made progress on confined native proc/sys mount setup and
  native-path VM disposal evidence. Added ProcRestrictions over the newly owned
  proc mount: sys, sysrq-trigger, irq and bus receive read-only nosuid/nodev/noexec
  cloned views through descriptor APIs. Only absent kernel-configuration entries
  are skipped; unexpected errors stop setup. No recipe path or raw mount handle
  is exposed, and no command runs during partial construction.
- v22 verifies all four read-only mountinfo entries and denied open-only access
  to sysrq-trigger without issuing a kernel-control write. Actual native image
  command, capability, timeout, reaping and clean filesystem persistence checks
  pass. Root tests, root/fixture Clippy and formatting pass; source/log/provenance
  are preserved. Review checked mount ownership, attribute application before
  attachment, configuration absence handling and evidence limits.
- M4 remains at zero adherence passes. Additional proc information masking,
  private devices/devpts/shm, runtime scaffolding, namespace initialization,
  active labeling policy and complete cleanup ownership remain unimplemented or
  unqualified. This change is not claimed to complete the sandbox.


- Previous goal turn made production progress on proc restrictions. Added
  CommandDevices: fresh detached tmpfs and private approved character-device
  inodes constructed by trusted initial-namespace setup, explicit mapped-root
  ownership/modes, and confined /dev attachment. No supervisor /dev inode is
  reused. Typed owners encapsulate mount and transfer descriptors.
- v23-v25 failed closed because O_PATH stdin prompted Rust startup to open an
  inheritable null FD. v26's readable reopen lost original detached-mount
  ownership. Preserved all runs. Final transfer queues the original mount FD
  over a private Unix socket, with exact canonical CBOR packet/count checks,
  no truncation, nonblocking receipt and CLOEXEC. Sender trust is an explicit
  setup-channel prerequisite; no descriptor audit exemption was introduced.
- v27 passed; v28 adds exact device-entry count, mapped-root ownership, null/full
  behavior and absence of runtime devices after image unmount/remount. Existing
  image commands, timeout, proc restrictions, reaping and filesystem checks pass.
  Root tests, root/fixture Clippy and formatting pass. Reviewed descriptor/mount
  lifetime, immutable ownership transfer, safe APIs and expected failure scope.
- M4 remains zero adherence passes. This does not complete private /dev: tty,
  sessions, devpts and shm remain. Packet-negative coverage, remaining proc masks,
  namespace init, active policy, partial setup and full cleanup ownership also
  remain. The fixture still uses external namespace setup adapters.


- Previous goal turn made progress on private device inodes and descriptor
  transfer. Added fresh session creation before root entry, a private tty inode,
  and private devpts/shm mounts prepared through safe filesystem-context APIs in
  the command user namespace. Targets are freshly created in the private device
  tree and resolved with no symlinks or mount crossings; descriptors retain
  ownership until attachment. Setup failures remain exit-only.
- v29 passes actual shell PTY allocation/release, absence of inherited controlling
  tty, fresh per-step shared-memory write/read, exact device-entry count and
  non-persistence after image unmount/remount. Existing capability, syscall,
  proc, timeout, reaping and filesystem checks pass. Root tests, root/fixture
  Clippy and formatting pass. Evidence, hashes and test output are preserved.
- Reviewed session ordering, mapped ownership, mount flags/configuration and
  bounded claim scope. PTY unlock/terminal ioctl compatibility is not proven by
  master allocation. M4 remains at zero adherence passes; namespace init/adapters,
  active LSM admission, partial-setup ownership, transfer negatives and complete
  integration/qualification remain outstanding.


- Previous goal turn made progress on private devpts/shm/session setup. Added
  maintained negative tests for the actual device descriptor handoff: malformed
  and truncated payloads, descriptor counts/truncation, wrong descriptor kinds
  and filesystem types, and an unqueued nonblocking receive. Socket-peer EOF
  proves rejected rights are released without racing global FD counts. Positive
  transfer retains independent ownership and CLOEXEC after channel closure.
- Private into_channel/receive methods support these tests while public entry
  points and wire semantics remain unchanged. An initial Stdio conversion test
  did not compile; the correction and scope are documented in
  validation/rewrite-command-integration/evidence/device-transfer-tests. No
  privileged operation occurred on the host. Full tests, final focused cases,
  Clippy and formatting pass; logs and source hashes are preserved.
- Review checked the shared production paths, truncation/RAII closure, borrowed
  descriptors and claim limits. This closes the listed packet-negative gap, not
  M4. Native namespace initialization, LSM admission, terminal compatibility,
  remaining proc information masks, scaffolding and full failure ownership are
  still outstanding; the M4 adherence streak remains zero.


- Previous conversational goal turn restated the dedicated-VM correction without
  changing authoritative state (no progress). Revalidated the worktree and resumed
  M4 with the remaining active-policy lifetime integration question.
- Source inspection confirms inode_xattr_skipcap receives only the attribute
  name; the later inode_setxattr/removexattr hooks receive the target. The current
  scoped exception cannot fold its filesystem check into skipcap. Preserve all
  hooks throughout command execution, including a supervisor-death interval.
- New disposable-VM v38/v39 tests kill and reap a production disabled-policy loader
  after closing witness descriptors, then reopen the exact same three links and
  both maps. v39 also loses the loader's private mount namespace. The existing
  bpffs mount in init's namespace retains the pins without a guardian. Both pass;
  source snapshots, raw logs and hashes are retained. This is a candidate lifetime
  mechanism, not proof of enabled-policy confinement or identity lifetime.
- Corrected policy documentation's obsolete independent-recovery requirement to
  the dedicated-VM model. Probe build, Clippy and formatting pass. Reviewed the
  absence of witness references across SIGKILL, bounded readiness and VM lifetime,
  disabled-only cleanup and claim limits. No host privileged operations occurred.
- M4 remains at zero adherence passes. Next integrate step-owned active policy,
  namespace/superblock lifetime and descendant quiescence before admitting actual
  commands; qualify supervisor death with that active policy before claiming the
  complete shutdown-interval contract. No guardian expansion is required.


- Previous goal turn made progress via be0ba89 and disabled-pin lifetime tests.
  Connected the production loader, policy asset and restricted command execution
  in a validation-only enabled-policy adapter. The adapter retains pins on every
  error and disables only after cgroup cancellation, helper reaping and descendant
  draining. Production activation/admission ownership is still pending.
- v30 failed closed on the fixture's mode-777 bpffs root; corrected it to 700.
  v31 passes policy-off denial, assigned ext4 set/read/remove, denial on private
  shared memory and an unauthorized attribute suffix, normal cleanup and timeout.
  v32 kills/reaps the supervisor, then verifies continued positive/negative label
  behavior in the surviving command before independent VM poweroff. No guardian
  is involved. Fixture-held original mount/user namespace/bpffs retain identities.
- v33 exposed channel-close-before-process-exit cancellation racing a successful
  helper. Added deadline-bounded pidfd waiting to ProcessObservation/Supervision,
  independently of channel loss and without reaping. A maintained unprivileged
  pipe-controlled child test covers timeout, exit and post-reap observation.
  v34 passes and additionally checks persisted labels after filesystem checking
  and read-only remount. Its regression delay was only in the test fixture;
  after user feedback, replaced that delay with explicit datagram release and
  an assertion that the helper is still alive after channel closure.
- Root tests (including fourteen compile-fail doctests), root/fixture Clippy and
  formatting pass. All failed/successful VM runs and source snapshots are retained.
  Review distinguishes channel completion, process exit and descendant quiescence;
  checks pin retention on activation/teardown error and confines claims to fixture
  topology. No host privileged operation occurred. M4 remains zero adherence
  passes: active production step ownership/admission, native namespace init,
  complete partial-setup cleanup and remaining sandbox coverage are unfinished.
- v35 passes the final explicit-release regression and label persistence checks;
  its evidence retains the tested production observation and fixture sources.
  No sleep is used to order the regression or to fix production completion.


- Previous goal turn made progress in fd41866 (active-label validation and pidfd
  completion). Added resident command NamespaceInit: image shell is a child;
  blocking waitpid(__WALL) reaps adopted descendants and retains shell outcome.
  PID-namespace exit stops remaining members; outer cancellation/drain remains
  required before policy teardown. No timing sleeps implement init or admission.
- Root entry now consumes the setup namespace owner and closes its restoration
  FD after successful pivot/detach. Resident init rejects all extra descriptors,
  including CLOEXEC handles, and becomes non-dumpable before shell spawn. Tests
  verify denied access to init executable/memory/maps/environment/fd through proc.
  These are confinement changes needed because exec no longer replaces PID 1.
- v36 passes orphan adoption/reaping while the shell remains alive, existing
  restricted command/policy checks and persistence. v37 revealed stale fixture
  mount-point discovery after a timeout; matched the owned mount identity rather
  than accepting every generated name. A root-test failure also required bounded
  EOF observation instead of an immediate nonblocking read during concurrent
  process spawning. Retained both failures; no production transport change.
- v38 passes the corrected fixture, shell status 37, successful shell exit with
  a live acknowledged child, timeout, label persistence and all prior checks.
  v39 confirms supervisor death, still-scoped active labels and independent VM
  shutdown with the resident init. Preserved source snapshots, kernel lifetime
  excerpts, logs and hashes. Root tests (14 compile-fail), root/fixture Clippy and
  formatting pass. No privileged host operation occurred.
- Review checked consuming namespace ownership, absence of outside descriptors,
  init proc visibility, wait ownership, shell status versus cleanup, and evidence
  boundaries. M4 remains zero adherence passes: native namespace creation, scoped
  root transfer, production active-policy step ownership/admission, complete
  partial-setup cleanup and remaining sandbox qualification are still outstanding.


- Previous goal turn made production progress in 568059b (resident namespace init).
  Added thread-affine CommandNamespaces with safe setns/unshare/credential APIs;
  it clears supplementary groups before joining the selected user namespace,
  becomes mapped root, creates fresh mount/PID/net/IPC/UTS namespaces and makes
  propagation private. Consuming spawn returns the existing process owner. Any
  partial transition remains exit-only in a dedicated trusted helper.
- v40 passes the command pipeline with native namespace construction. v41 checks
  exact namespace identities, mapped credentials, empty supplementary groups and
  private propagation, plus regular-file and synchronized multithreaded-entry
  rejection. v42 passes all command/policy/timeout/failure checks after runtime
  tools are removed from the guest command path. v43 confirms supervisor death,
  continued scoped labeling and independent VM poweroff through this native path.
- Root tests, root/fixture Clippy and formatting pass. Source snapshots, logs and
  hashes are retained. Added a compile-fail contract for cross-thread spawning;
  the entry/spawn interval forbids other child/thread creation, and root entry
  independently requires PID 1 before running image code. Review checked namespace
  ownership, credential ordering, descriptor closure, partial errors and claims.
- M4 remains at zero adherence passes. Initial mapped-user-namespace allocation
  still uses an external fixture holder; production mapping/pinning, scoped root
  transfer and the full active-policy/admission/cleanup aggregate remain pending.
  No guardian or separate cross-invocation recovery requirement was introduced.
- Final documentation gate passes all fifteen compile-fail doctests, including
  the new namespace thread-affinity contract; its log is retained with v43.


- Previous goal turn made progress in 38d35aa (native command namespaces). Added
  immutable IdRange validation and MappedUserNamespace creation: a short-lived
  trusted creator unshares NEWUSER, then exchanges identities; the parent writes
  empty UID/GID maps once, retains namespace ownership and reaps the released
  creator before returning. Mapping/reaping failures retain process cleanup
  ownership. Initial E0382 move ownership error was corrected before execution.
- Added an internal process proc-directory accessor that checks the pidfd's Pid
  in the current procfs instance before using the owned child's numeric path.
  Kernel source confirms that field's namespace-relative meaning. The wait owner
  prevents PID reuse under the documented exclusive-reaping/SIGCHLD contract.
- v44 passes fresh native mapped namespaces per step with creators already reaped.
  Existing namespace pins retain identities through policy lifetime; corrected
  their obsolete documentation requiring serialized records. v45 adds rejection
  of a creator that never unshared, verifies the original mapping unchanged and
  child reaped, and confirms group changes remain allowed in command namespaces.
- v46 passes supervisor death with no external namespace holder: native pins and
  scoped labels survive until independent VM poweroff. Root tests (15 compile-
  fail), root/fixture Clippy and formatting pass. Preserved source snapshots,
  logs, hashes, kernel source and the initial compile-error explanation. All
  privileged tests ran only in disposable QEMU guests.
- Review checked single-write map semantics, range boundaries, pidfd/proc identity,
  creator release/reaping, pin lifetime and post-quiescence release. M4 remains
  zero adherence passes. Production policy/admission aggregation, scoped handle
  transfer, complete late-error cleanup and remaining runtime/qualification work
  are still pending; no complete sandbox claim is made.

- Previous goal turn made progress by replacing the setup-loss test's 20 ms
  delay with receipt of the actual setup request before upstream closure. The
  focused test passes. Remaining deadline polling checks actual state; elapsed
  time must never establish readiness, quiescence or cleanup success.
- Added consuming Mount<Attached>::launch -> Mount<InUse>. The trusted setup
  child's cwd is the owned mount; namespace cloning retargets cwd into the new
  mount namespace, and CommandRoot captures it afterward. Removed fixture mount
  directory discovery and root-path arguments. No mount-path/descriptor getter
  was added. Failed launch returns Attached; failed wait returns InUse with the
  child and mount retained; only reaping restores Attached for explicit close.
- v47 validates the native cwd handoff. v48 adds an unrelated mount-name decoy,
  failed launch ownership, and an IPC-blocked child proving immediate wait
  timeout and retry without timing assumptions. v49 additionally validates
  direct-child termination and retained mount identity. v50 passes supervisor
  death, still-scoped labeling and independent VM shutdown with this topology.
  All privileged operations ran in disposable guests. Preserved source snapshots,
  raw logs, hashes, kernel cwd-retargeting source and full artifact locations.
- Root tests (16 compile-fail), root/fixture Clippy and formatting pass. Review
  checked namespace/cwd ownership, failure return types, descriptor closure,
  direct child versus descendant lifetime, and limits of destructor cleanup.
  InUse is not a descendant-quiescence or filesystem-finalization receipt; the
  production enclosing step must enforce those before teardown/publication.
- M4 remains zero adherence passes. Namespace pin paths and policy activation
  still use fixture adapters. Production step admission/policy aggregation,
  complete late-error cleanup, remaining runtime support and qualification are
  unfinished. No new guardian or cross-invocation recovery service is required.

- Previous goal turn made progress in 901163b (owned mount handoff). Inspection
  of activation ownership confirms a naked enable method would not enforce
  assigned namespace/mount lifetimes or descendant teardown. The fixture active
  adapter remains intentionally unavailable as a production activation API.
- Replaced direct-child wait and destructor-reap sleep polling with pidfd event
  observation followed by status collection. Identified helpers reuse their
  received pidfd; Running children obtain one while exclusively wait-owned.
  Normal SIGCHLD zombie semantics and exclusive reaping are explicit prerequisites
  (already required by Child and numeric termination ownership). Failures retain
  child ownership; no elapsed delay or channel closure constitutes process exit.
- Process tests now hold a child on pipe input rather than a long sleep, prove
  immediate-deadline owner retention, then separately test explicit EOF release
  and cancellation. Root suite (16 compile-fail), Clippy, formatting and v51's
  disposable-VM normal/timeout/failure pipeline pass. Preserved kernel pidfd source,
  fixture/source snapshots, hashes and logs. No privileged host operations.
- Review checked pidfd acquisition before reaping, error/timeout ownership,
  SIGCHLD assumptions, and observation versus collected status. This is not a
  full milestone adherence pass. Cgroup population and adopted-child drain still
  use bounded condition polling; production policy/admission aggregation and
  complete failure cleanup remain unfinished. M4 remains zero adherence passes.

- Previous goal turn made progress in 1c2b381 (pidfd direct-child waits).
  Replaced cgroup cancellation's sleep polling with a private DomainWatch owner
  for one cgroup.events open description. Each bounded read begins at offset zero;
  populated=0 alone permits directory removal. Otherwise poll waits on PRI until
  the shared teardown deadline, then rereads. EINTR retries with the same deadline.
- Source review confirms kernfs records its event counter before seq_show and
  compares it after poll registration, preserving changes between read and wait.
  Ordinary IN readiness is deliberately excluded; PRI|ERR is kernfs's normal
  notification, whereas invalid/hung-up descriptors fail. Errors retain the
  enclosing cancellation owner; notifications do not imply descendant reaping.
- Root tests (16 compile-fail), Clippy, formatting and v52's disposable-VM normal,
  timeout and failure pipeline pass. Preserved raw evidence/source snapshots,
  full artifact hashes and kernel source/documentation excerpts. This integration
  run checks cancellation results, not deterministic injection at every possible
  read/poll boundary; that ordering argument is grounded in the retained source.
- M4 remains zero adherence passes. Adopted-child drain and fixture readiness
  polling remain; production policy/admission ownership and complete failure
  cleanup still need implementation. No milestone-completion claim is made.

- Previous goal turn made progress in d874300 (cgroup event waits). Removed
  adopted-child drain sleep polling too: after nonblocking waitpid reports live
  children, open a pidfd for a child selected from procfs, check the pidfd's
  procfs-visible PID, await exit, then resume status collection. No signal mask
  changes are introduced. Exclusive single-threaded reaping and normal SIGCHLD
  zombie semantics remain mandatory. Successful collection is bounded by the
  same deadline and cumulative count is retained on failure.
- The procfs list selects a wait target only. It is not an emptiness/completion
  oracle: only waitpid ECHILD completes drain. Kernel source documents that the
  general children iterator is not an atomic snapshot; the code does not infer
  completion from an empty/inconsistent list, and does not concurrently reap.
  Procfs/PID inconsistencies and pidfd errors fail with ownership retained.
- Added a disposable-VM reaper probe: explicit pipe blocks a relinquished child,
  an immediate drain deadline fails retaining the owner, EOF releases the child,
  retry collects exactly one status and reaches ECHILD. v53 passes this and the
  normal/timeout/failure integration sequence. Root tests (16 compile-fail),
  root/fixture Clippy and formatting pass; evidence and source excerpts preserved.
- Review checked status versus readiness, lifetime assumptions, retry counts,
  deadline ownership and procfs limitations. M4 remains zero adherence passes;
  production admission/policy ownership, late-error cleanup and remaining runtime
  qualification are unfinished. Fixture readiness still uses bounded state
  polling; no production process/domain cleanup sleeps remain.

- Previous goal turn made progress in 022f6ba (event-driven adopted-child drain).
  Returned to production activation prerequisites: pinning previously discarded
  the mapping description, forcing the fixture to rediscover the namespace from
  a path and restate mapped root UID 100000 when creating policy scope.
- Replaced MappedUserNamespace with sealed UserNamespace<Mapped> and
  UserNamespace<PinnedNamespace>. Consuming pin preserves the immutable UID
  mapping together with the kernel-held namespace owner. Only the pinned state
  derives label scope from that owner and mapping. Release failures retain either
  the typed mapped pin for retry or only the unmounted target for unlink.
- Updated the fixture to derive policy scope from the typed namespace owner.
  Raw LabelScope remains a description, not activation authority; this change does
  not permit commands or claim to couple live policy to all resource lifetimes.
  Added compile-fail checks for pre-pin policy description and repeated pinning.
- v54 passes normal/timeout/failure execution; v55 passes supervisor death with
  scoped labels still enforced through independent VM poweroff. Root tests (18
  compile-fail), root/fixture Clippy and formatting pass. Preserved snapshots,
  raw logs, hashes and full artifact locations. No privileged host operations.
- Review checked consuming transitions, mapping provenance, descriptor exposure,
  release-error ownership, and description-versus-authorization boundaries.
  M4 remains zero adherence passes. Assigned filesystem lifetime ownership,
  production policy/admission aggregation, scoped namespace handoff and full
  late-error cleanup still need integration. Original mount lifetime and enabled
  policy are still fixture adapters; no complete sandbox claim is made.

- Previous goal turn made progress in 24e1adc (typed namespace/mapping lifetime).
  Added FilesystemPin: a non-recursive bind mount under the protected initial
  mount-namespace directory. It independently retains the selected superblock
  across source unmount and supervisor death, without exposing root descriptors.
  Metadata checks confirm retained device/inode before explicit release.
- Pin creation disarms recursive directory cleanup before mounting. There are no
  fallible operations after the successful bind. Failed mount returns an empty
  target; failed release retains either the attached owner or the unmounted
  target. Destructors use rmdir only, never traverse image contents or detach a
  live pin. Active pins deliberately remain until explicit release or VM disposal.
- Fixture policy target identity now uses pinned filesystem metadata. Its normal
  cleanup releases filesystem and namespace pins only after policy disable and
  descendant drain. This ordering is still fixture-enforced; the production step
  aggregate is not implemented. Pinning alone neither identifies the superblock's
  owning user namespace nor grants policy/admission authority or exclusive writes.
- v56 rejects a regular-file source without residue, unmounts the original ext4
  source, verifies the pin retains the same filesystem, restores the source mount,
  and releases the pin. Normal/timeout/failure pipeline passes. v57 kills the
  supervisor, verifies its filesystem pin remains attached, unmounts the original
  source, and checks surviving command labeling through the retained pin before
  independent VM poweroff. Both pass; no privileged host operations occurred.
- Root tests (18 compile-fail), root/fixture Clippy and formatting pass. Preserved
  source/log/hash evidence and a retrospective note for the initial fixture-only
  missing-dependency compile error. Review checked leak-on-drop identity lifetime,
  non-recursive cleanup, success/failure ownership and descriptor boundaries.
  M4 remains zero adherence passes: production policy/admission aggregation,
  scoped namespace handoff, late-error cleanup and runtime qualification remain.

- Previous goal turn made progress in 95cde7c (kernel-held filesystem pins).
  Added LabelPolicy<BoundDisabled>: consuming bind verifies namespace/root-UID
  correspondence and exact distinct device membership, then owns namespace and
  filesystem pins alongside the disabled policy. Binding failures return all
  inputs. Only successful close returns PolicyIdentities for subsequent release.
  This does not infer superblock owner namespace from stat: trusted filesystem
  construction must supply that provenance before production activation.
- Fixture activation now holds this aggregate rather than separately owned pins.
  Added missing-pin and wrong-filesystem rejection/retry checks. After commands
  are drained and config disabled, a benign extra bpffs directory deliberately
  makes close fail after partial pin removal. The fixture verifies identity pins
  remain, removes the blocker and retries cleanup before releasing identities.
- v58 passes the new normal/timeout/failure checks. Clippy found a large error;
  boxed BoundCloseFailure without dropping ownership. v59 passes that revision;
  v60 confirms supervisor-death pin/label persistence through VM shutdown.
- Source review then found a typestate defect: partial close returned a bound
  ready-policy type, which could become reactivatable once activation is added.
  Corrected it to BoundCleanup, a cleanup-only owner with retry and no return to
  ready policy. Preserved pre-correction evidence with explicit review notes.
  v61 passes the corrected retry path and integration sequence. Death-path
  behavior from v60 is unchanged by this cleanup-only API correction; v60 is
  explicitly retained as evidence for its own source snapshot, not relabeled.
- Root tests (20 compile-fail), root/fixture Clippy and formatting pass. Source
  snapshots, raw logs, hashes, diagnostic/review notes and full artifacts remain
  preserved. No privileged host operations occurred. Review covered partial
  teardown, identity ownership, mismatch handling and authorization boundaries.
- M4 remains zero adherence passes. Production activation/admission and command
  quiescence must be integrated with this bound owner; active configuration is
  still fixture-only. Filesystem owner provenance, scoped namespace transfer,
  complete late-error cleanup and runtime qualification remain outstanding.

- Previous goal turn made progress in 729c6a2 (bound disabled policy ownership).
  Integration inspection found registration lacks an independent command gate.
  Added Registered -> Admitted protocol state and descriptor-free canonical CBOR
  admission/acknowledgment packets. A trusted helper waits after joining its
  domain and before executing any image code. EOF, timeout and wrong-phase
  messages fail closed; descriptor/opcode combinations remain strictly checked.
- ContainedHelper now carries its supervision state. Consuming admission yields
  the admitted owner; admission failure returns cancellation-only ownership of
  the process and domain. A failed ACK can follow delivered admission, so error
  never implies that execution did not start. Added a compile-fail double-admit
  contract and host protocol tests for explicit admission, timeout and wrong phase.
- Added a VM case that expires admission before send, cancels/reaps/drains the
  blocked helper, removes policy and verifies no command marker or children.
  Existing command flows now explicitly admit after policy setup. This gate is
  a low-level trusted-caller protocol, not proof of policy/sandbox readiness;
  the enclosing production step must still enforce those prerequisites.
- Validation mistake preserved as v62: fixture build failed E0515 on returning
  a borrowed error through Box<dyn Error>, but the runner was incorrectly started
  and copied the preceding binary. The new mode failed in old code. v62 is
  explicitly INVALID admission evidence, with raw failure and binary provenance.
  Corrected fixture fatal error conversion; production errors retain their owners.
  Runner now requires a successful build and preserves build.txt before guest
  construction, preventing a failed build from silently reusing a prior binary.
- v63 passes admission failure cleanup and normal/timeout/command-failure checks.
  v64 passes supervisor-death pin/label persistence through independent VM poweroff.
  Root tests (21 compile-fail), root/fixture Clippy and formatting pass. Preserved
  all snapshots, logs, hashes and the invalid run. No privileged host operations.
- Review checked packet phases/descriptors, lost acknowledgments, cancellation
  ownership, helper ordering and evidence validity. M4 remains zero adherence
  passes: production activation must be tied to this gate and same-domain drain;
  owner provenance, scoped transfer and complete late-error handling remain.

- Previous goal turn made progress in 2ae03fd (explicit command admission).
  Added production Step with sealed Ready, Running, Cancelling, Reaping, Draining,
  Quiescent and Finishing states. It consumes bound policy, the registered helper
  and the preinstalled child reaper. It obtains the config descriptor directly
  from the loaded object, enables the exact subject before admission, and never
  exposes active policy, namespace/filesystem owners or map handles to callers.
- Failure transitions retain the appropriate owners. Admission failure closes
  admission and returns cancellation-only state; activation failure retains a
  still-unadmitted ready owner for cancellation. Cancellation/reaping/draining/
  record cleanup failures retain their actual phase. Only this same domain's
  completed drain and record cleanup permit config disable and return a disabled
  policy for cleanup. Dropping unfinished state retains all policy/identity pins
  through VM disposal; best-effort child Drop is not a completion receipt.
- PinDirectory now captures/verifies deployment-init mount-namespace placement.
  A private helper namespace is rejected before creating policy pins. Trusted
  construction must still supply a bpffs attached in that namespace and retain
  it until VM shutdown, excluding external privileged mutation or descriptor
  paths into another namespace's bpffs. No independent guardian is introduced.
- v65 passes production activation/admission, normal execution, shell failure,
  execution timeout, admission timeout and cleanup retry. v66 passes supervisor
  death, surviving scoped label checks and pin lifetime until independent VM
  poweroff. Boxed one oversized internal error after Clippy's diagnostic; retained
  the earlier evidence and diagnostic note. No privileged host operation occurred.
- Removed direct BPF map activation from the fixture and its direct libbpf-rs
  dependency. The negative baseline owns permanently disabled policy; all enabled
  cases use Step. v67 adds cancellation of Ready before any admission and passes
  that plus the complete normal/timeout/failure suite. Root tests (24 compile-fail),
  root/fixture Clippy and formatting pass. Snapshots/logs/hashes are preserved.
- Review checked policy-update ordering across hooks, same-command ownership,
  cancellation-only failures, cleanup retry, namespace placement, immutable scope
  and safe leak-on-abandon behavior. This is not a full milestone adherence pass.
  M4 remains zero: production helper/scope construction and scoped namespace
  transfer, RAM/storage assembly, complete late-error/fault injection coverage,
  fixed orchestration deadlines and remaining runtime qualification are pending.
  The existing recorded-domain adapter remains transitional; this does not add
  a cross-invocation recovery service or require a guardian. The full CLI/pipeline,
  distro/platform qualification and retirement milestones remain outstanding.

- Previous goal turn made progress in 379d299 (production policy/command Step).
  Source review identified a remaining placement assumption: checking the caller's
  mount namespace did not establish that an opened control directory belonged
  to a mount attached there. A retained descriptor can reference foreign bpffs
  after restoration to init's namespace, undermining the intended pin lifetime.
- Added InitMountNamespace::verify_attached: verify current init namespace, read
  STATX_MNT_ID from the pinned descriptor, require the kernel-reported mask, and
  find the same ID in real procfs thread-self/mountinfo. Namespace/RecoveryDirectory
  and policy-pin construction require it; Step's config-handle acquisition also
  rechecks policy placement before activation. No fallback or inferred placement.
- Kernel source confirms statx and mountinfo use the same mount ID, allocated from
  the global mount ID registry and freed during mount destruction. The descriptor
  retains the selected mount during verification. This is present attachment
  evidence, not a promise against later privileged unmount: the independent VM
  environment still must retain its control mounts until shutdown.
- v68 creates private bpffs/tmpfs mounts in a disposable guest, retains descriptors,
  restores init's namespace, and verifies both constructors reject the foreign
  mounts through descriptor paths. It then passes the normal production-step,
  admission-failure, cancellation, timeout, shell-failure and cleanup retry suite.
  Root tests (24 compile-fail), root/fixture Clippy and formatting pass. Retained
  source excerpts, snapshots, raw logs, hashes and full artifacts. No privileged
  host operation occurred; no supervisor-death rerun was needed for this placement
  check, and no new lifetime claim beyond the existing death evidence is made.
- Review checked mount-ID view, descriptor retention, fail-closed missing-ID/error
  behavior and trusted mutation assumptions. M4 remains at zero adherence passes;
  scope handoff, workspace/storage construction and remaining lifecycle/runtime
  qualification are still outstanding. No goal or milestone completion claimed.

- Follow-up to the user's prohibition on sleep-based race fixes: inspection found
  no sleeps in current production Rust, but two timed holds in host lifecycle
  fixtures and two readiness-marker polling loops in the disposable-VM fixture.
  Replaced the host holds and external /bin/sleep unresponsive peers with local
  socket reads whose peer remains owned until process termination. Successful
  identity/supervision handshakes still establish readiness; elapsed time never
  establishes readiness or completion. Timeout tests intentionally retain bounded
  deadlines. All 15 identity/supervision tests, targeted Clippy and formatting
  pass. The first compile attempt used rustix::pipe, which is feature-disabled;
  corrected to std UnixStream without changing dependencies. No privileged host
  operations. VM readiness polling remains outstanding, as does integration and
  validation of the uncommitted CommandSetup descriptor transport. This is a
  focused fixture correction, not a milestone adherence pass; M4 remains zero.

- Previous goal turn progressed in 340be44. Added CommandSetup, a consuming trusted
  namespace/device descriptor bundle, and integrated it in the inner setup hop.
  Idmapping and namespace entry use the same opened namespace identity; no inner
  namespace pathname argument/reopen. Received rights are CLOEXEC, exact framed
  packets require two descriptors, and malformed/truncated inputs drop all rights.
  Device handles remain internal to isolation. Failed/partial namespace entry is
  exit-only; this bundle is neither admission nor a full isolation receipt.
- v69 normal and v70 supervisor-death disposable-VM suites pass. Host protocol
  tests verify malformed framing/cardinality/object rejection, descriptor closure,
  nonblocking missing-offer failure, identity preservation and CLOEXEC. First test
  compilation attempted unsupported Stdio-to-OwnedFd conversion; preserved the
  failure and tested reception through a direct socket instead. Production sender
  is covered by both VM runs. Full root tests (24 compile-fail), root and fixture
  Clippy and formatting pass. Evidence includes raw serial, sources, hashes and
  snapshot limitations; no privileged host operations or new qualification claim.
- Focused review checked transfer ordering, error ownership, lack of descriptor
  escape to commands and namespace/device lifetime. nsfs/tmpfs validation is not
  provenance authentication: trusted constructors must select the scope; setns
  validates namespace kind. First helper still opens the fixture namespace path.
  Scope-bound production construction, readiness-loop replacement, RAM/storage
  assembly and full lifecycle/fault coverage remain unfinished. M4 stays at zero
  adherence passes; subsequent milestones and the overall goal remain active.

- Previous goal turn progressed in fa8ffbc. Replaced both disposable-VM Rust
  readiness sleeps with a fixture-owned inotify watcher: register before checking
  marker presence, then poll events under an absolute deadline and recheck state.
  Unrelated events never imply readiness; each bounded drain returns to deadline
  and state checks. Peer loss without filesystem events is observed by the setup
  deadline. Added host tests for existing/queued markers, concurrent creation and
  unrelated events. This observes marker existence only; final content/ownership
  assertions still occur after execution cleanup.
- v71 preserves the initial compile failure and source (a removed binding was
  still needed for final assertions); runner aborted before boot. Corrected v72
  normal and v73 supervisor-death suites pass. Root tests (24 compile-fail), fixture
  tests, both Clippy checks and formatting pass. Also replaced the remaining
  /bin/sleep unit-test hold in process supervision with a stdin-blocked child.
  No privileged host operations; source/evidence/hashes retained. No sleeps remain
  in active production Rust, host tests or this Rust VM fixture; historical files
  were not rewritten. Shell coordination is not claimed migrated here.
- Focused review checked watch-before-check ordering, deadline bounds, nonblocking
  drainage, spurious-event handling and claims versus VM coverage. No milestone
  adherence pass: M4 remains zero, with full scope construction, RAM/storage and
  remaining lifecycle/fault coverage outstanding. The overall goal remains active.

- Previous turn progressed in 6b7b122. Began production storage with Workspace:
  one owned detached tmpfs, private root, nosuid/nodev/noexec, environment-managed
  capacity, no disk fallback. Anonymous Scratch leases borrow the workspace and
  expose Read/Write/Seek, sizing, metadata and sync without public raw descriptors
  or Clone. Intermediate fsopen/fsmount descriptors use RAII on every error; no
  mountpoint attachment or global name requires cleanup. This is temporary RAM
  storage, not a finalized layer, command lease or successful output receipt.
- v74 disposable-VM probe verifies anonymous files, filesystem sharing/inode
  separation, permissions, root ownership, sparse zero-fill and I/O; mountinfo is
  unchanged after release. Existing normal suite also passes. Root tests now have
  25 compile-fail cases including workspace lifetime; both Clippy checks and
  formatting pass. Evidence/sources/raw logs/hashes retained. No privileged host
  operation, disk fallback, or new supervisor-death qualification claim.
- Focused review checked descriptor cleanup, borrowed lifetime, absent raw-handle
  escape and environment-controlled tmpfs behavior. Future device users must own
  kernel leases through quiescence; a Rust borrow alone does not prove this. Full
  build-wide integration (context, writable snapshots, intermediate/output storage),
  fault injection and storage/device lifecycle remain outstanding. M4 has zero
  clean adherence passes; no milestone or goal completion claimed.

- Previous turn progressed in d81f7d4. Connected CapturedContext staging to the
  owned workspace with an internal descriptor-based materialization method.
  Workspace::materialize clones only internal parent authority; no path or raw
  writable handle is public. The staged context owns its parent reference through
  recursive cleanup, so dropping the initial Workspace cannot invalidate its
  proc-fd cleanup authority or destroy its underlying tmpfs prematurely.
- v75 verifies committed context bytes, same-filesystem placement with scratch,
  successful read and cleanup after the original workspace owner is dropped,
  plus the existing normal command suite. Root tests (25 compile-fail), root and
  fixture Clippy and formatting pass. Exact Git fixture bundle, commit identity,
  sources, raw logs and hashes retained; no privileged host operations.
- Existing StagedContext::read_only uses an attached bind/remount; it is not yet
  qualified against this detached workspace. Do not claim a read-only workspace
  context or completed context pipeline. That transition, whole-build storage and
  kernel/device lifetimes remain required. This focused ownership change is not
  a milestone adherence pass; M4 remains zero. Context integration changes must
  be included in subsequent adherence review, not inherit M3's earlier evidence.

- Previous turn progressed in ac69bba. Integrated detached read-only checkout:
  StagedContext::read_only clones only its selected tree, sets read-only/nosuid/
  nodev/noexec/private attributes, closes the old writable root descriptor and
  publishes the owned view. Private backing and parent remain for cleanup. Setup
  failures drop the incomplete detached view and return the untouched staged
  owner for retry/cleanup; no attached-mount recovery is needed. Successful close
  releases the view and removes backing names. Existing read-only file handles
  can retain their inodes without regaining writable authority or blocking cleanup.
- v76 proves workspace-backed freeze, EROFS mutation rejection before and after
  cleanup with a retained reader, exact committed bytes and unchanged mountinfo.
  Benign descriptor exhaustion produces EMFILE, retains staged ownership, and
  succeeds on retry after restoring the limit. The normal command suite passes.
  Root tests (25 compile-fail), root/fixture Clippy and formatting pass. Updated
  the old context harness's API expectations; preserved stale-lock check failure,
  refreshed its lock offline and compiled it. No separate old-harness VM rerun
  or new supervisor-death claim. Evidence, Git bundle and exact sources retained.
- Focused review checked nonrecursive cloning, attribute success before publication,
  old writable descriptor closure, backing cleanup order, thread-affine ownership
  and failed-transition retry. The supervisor still owns private writable backing
  solely for cleanup, never supplied to commands. Whole-build assembly and further
  cleanup fault coverage remain outstanding. This changes M3 context lifecycle:
  its prior clean passes are historical, not qualification of this implementation;
  the changed scope needs three new clean adherence passes before advancement.
  M4 remains zero and the overall goal remains active.

- Previous turn progressed in 522688f. Context lifecycle review found that
  TempDir::close consumed cleanup authority on removal failure. This violated the
  explicit recovery requirement. Replaced it in staged/read-only cleanup with
  BackingDirectory: borrow the private pathname during removal, retain TempDir
  and parent FD on error, disarm redundant traversal only after success. Public
  ContextCleanupFailure is cleanup-only and supports retry; it cannot recreate
  a staged/read-only state after potentially partial deletion. Discarding it still
  runs ordinary best-effort TempDir cleanup before releasing the parent.
- v77 injects EMFILE during read-only cleanup after dropping Workspace, verifies
  the specific failure, restores the limit and successfully retries. The live
  read-only reader still rejects mutation. Normal suite, root tests (25 compile-
  fail), both Clippy checks and formatting pass. Evidence/source/Git bundle/hashes
  retained. No privileged host work. No claim of all partial deletion failures.
- The finding resets the changed context scope's adherence streak to zero. This
  fix and test are not clean adherence passes. Remaining build-wide integration,
  storage/device assembly and lifecycle qualification still prevent M4 completion;
  the full goal remains active.

- Previous turn progressed in 395ba80. Changed-context review against DESIGN §3,
  SANDBOX-DESIGN §2.1 and the ownership/error contract found another failure gap:
  population errors returned only ContextError and dropped the partial directory,
  making cleanup best-effort with no retained recovery authority. Added distinct
  ContextMaterializeFailure::Before/Partial results. After allocation, failed root
  opening or entry population returns ContextCleanup; it cannot expose a valid
  StagedContext. Cleanup failure retains its existing retry-only owner.
- A host regression builds a valid Git tree with a short first filename and a
  256-byte second filename, captures it, stages on ordinary /dev/shm, verifies
  ENAMETOOLONG after the first file exists, and explicitly removes the partial
  directory through returned ownership. No namespace/mount/capability operation
  occurred on the host. Full root tests (25 compile-fail), root/fixture Clippy,
  old fixture compilation and formatting pass. Logs preserved under evidence/
  context-population-review; kernel mount behavior was unchanged, so no new VM
  behavior is claimed and prior v77 evidence is not represented as this build.
- Reviewed state construction, consuming failure transitions, root/parent drop
  order, read-only access confinement and cleanup API exposure. Finding/fix means
  no clean pass; M3's summary table is now explicitly reopened at zero rather than
  incorrectly displaying its historical three passes as current. Restore M3's
  three clean reviews before advancing the changed foundation. M4 remains zero;
  all later milestones and the full goal remain outstanding.

- Previous turn progressed in a7f7217. Completed the first clean revised M3
  self-review on frozen scope a7f7217: requirements, changed context ownership,
  unchanged foundation contracts, Rust style, failure recovery, test assertions,
  provenance and preservation. Verified 32 recorded hashes. No source/test edit
  or new finding; details in preservation/MILESTONE-3-CONTEXT-REVISION-ADHERENCE.md.
  Streak 1/3; M3 remains gated and M4 has zero clean passes. This is a review,
  not another test run or independent/full sandbox qualification.

- Previous turn progressed in 19442b5. Second revised M3 clean self-review used
  failure/evidence-first traversal on unchanged a7f7217: partial population,
  freeze failure, cleanup recovery, cache/decoder failure distinctions, parser
  rules, public authority, Rust style and preservation. Context negative-compilation
  diagnostics confirm intended E0505/E0277/E0599 restrictions; retained the log.
  No finding or implementation change. Streak 2/3; M3 remains gated, M4 zero.

- Previous turn progressed in 5db425f. Third revised M3 clean self-review completed
  on unchanged a7f7217: acceptance criteria, public authority, style, ownership and
  recovery, evidence limits and preservation. Forty legacy source hashes match
  baseline; original archive remains present. No new finding. Revised M3 accepted
  at 3/3; M4 can resume but retains zero clean passes. Storage/device assembly,
  full lifecycle coverage and later milestones remain unfinished. Goal active.

- Previous turn progressed in db69a8e and restored the M3 gate. Resumed M4 by
  inspecting pinned devmap-linux and carapace assembly: Control does not own kernel
  mapping lifetime, and create/load/activate are separate fallible operations.
  Added sealed Mapping<Created>/Mapping<Active> owners for the initial dm-zero.
  Unique random names are created without adopting existing devices. Invalid
  extents, load and activation failures return cleanup-only ownership. Explicit
  removal is immediate and retains ownership on failure; Drop requests deferred
  removal as best-effort only. Diagnostic device numbers are not lifetime leases.
- Added devmap-linux at the same already-pinned devmap revision, without changing
  dependency revisions. v78 guest-only probe verifies simultaneous unique IDs,
  zero reads, EBUSY cleanup retention/retry and cleanup of a rejected extent's
  created device. The existing normal suite passes. Full root tests (26 compile-
  fail), root/fixture Clippy, fmt and Python syntax checks pass. Source/raw logs,
  exact inputs and artifact hashes preserved. No privileged host operations.
- Focused review checked resource ownership, collision behavior, state transitions,
  no arbitrary public table/control authority and cleanup failure retention. This
  is not a milestone adherence pass. Blocking ioctls still require fixed-deadline
  orchestration; late setup faults, deferred Drop timing and device lifecycle under
  supervisor death need further coverage. Loop/cache-backed verity layers and
  writable snapshots are not assembled yet. M4 remains zero; overall goal active.

- Previous turn progressed in b127f47. Inspected pinned lodown 31f8951: clear may
  merely schedule detachment, cloned file descriptions do not add independent
  loop users, and attach's convenience helper retries indefinitely. Added an
  owned LoopAttachment for Scratch with a single get_free/open/configure attempt,
  atomic autoclear, no public backing/raw handle, and backing-return only after
  flush, clear and explicit unbound status. Deferred detach returns retry-only
  ownership. Attach failure returns the untouched scratch owner. Added lodown at
  the existing carapace-compatible pinned revision; no unsafe application code.
- v79 preserves fixture E0515 compilation failure (no boot); reporting now converts
  borrowed fixture errors to strings before the local workspace is destroyed.
  v80 disproved the assumption that zero-length loop configuration fails. Added
  explicit positive 4096-byte-multiple validation. v81 passes invalid extent
  retention, loop/file byte visibility, deferred detachment with a second holder,
  retry after holder release, and reattachment, plus the normal command suite.
  Root tests, root/fixture Clippy, formatting and Python checks pass. Both failures,
  source snapshots, input bundles, raw serial and hashes retained. No privileged
  host operations occurred.
- Focused review checked single-writer authority, descriptor-vs-open-file lifetime,
  partial error ownership and no false detach success. Kernel backing references
  survive Drop until autoclear, but that is not successful sealing/quiescence.
  No competing trusted actor may reconfigure the owned loop. Bounded ioctl setup,
  late-error recovery, cache/verity/snapshot composition and crash qualification
  remain required. M4 remains zero; no adherence or overall completion claim.

- Previous turn progressed in d241f67. Added persistent SnapshotMapping using
  pinned devmap-snapshot, 4096-byte chunks, P metadata and disabled discard. The
  snapshot consumes LoopAttachment, borrows its read-only origin and returns the
  backing owner only after immediate mapping removal. Failed setup/close retains
  cleanup-only authority plus backing; caller detaches the returned loop before
  regaining Scratch. Mapping's target activation helper remains storage-private.
- Review found two earlier ownership designs inadequate: shared borrows permitted
  multiple snapshot writers (v82); exclusive borrows released backing access on
  wrapper Drop while deferred kernel mappings could survive (v83). Both passing
  ordinary-I/O runs and exact sources are retained with explicit superseded notes.
  Final v84 uses ownership, and two negative-compilation checks produce E0382 for
  backing reuse/second writer. It verifies snapshot writes, EBUSY cleanup retry,
  removal/loop detachment, reactivation over fresh dm-zero and persisted bytes.
  Normal suite, root tests (28 compile-fail), root/fixture Clippy, formatting and
  Python checks pass. No privileged host operation; evidence/hashes preserved.
- This is an M4 ownership component, not M5 bootstrap/sealing advancement. Snapshot
  invalidation/overflow checks, finalization, cache-backed chain assembly, fixed
  setup/teardown bounds and broader faults/crash coverage remain outstanding. No
  successful artifact receipt is produced. Findings keep M4 at zero adherence
  passes. Goal remains active with all later milestones required.

- Previous turn progressed by replacing two supervisor-death fixture sleep polls
  with the existing inotify readiness observer. This turn verified the changed
  scenario and pending snapshot health work. SnapshotMapping now exposes an
  immutable usage observation, rejecting invalid/overflow/unknown/inconsistent
  status. This observation is explicitly not a quiescence or sealing receipt.
  v85 passes healthy writes/reactivation and a bounded 64KiB COW exhaustion test:
  write/flush fails and status is rejected, followed by owned cleanup. The normal
  suite passes; root tests, root/fixture Clippy and formatting checks pass.
- v86 passed event-based supervisor-death readiness, but source review identified
  non-atomic publication of a marker whose contents are read immediately. Changed
  publication to write-then-rename. v87 correctly failed its readiness deadline
  because mv was absent from the command root; added it and its libraries to the
  disposable fixture. v88 passes policy/identity retention after supervisor death
  and independent VM poweroff. Four readiness tests pass, including rename
  publication, without sleeps. Failed and intermediate evidence is retained.
- No production synchronization sleeps were introduced. Fixture PID1's existing
  pre-reset delay and terminal sleep remain shutdown mechanics, not readiness
  evidence; historical harnesses are preserved unchanged. Shell command hold loops
  remain fixture-only. This is focused source/error-path review, not a milestone
  adherence pass. M4 stays at zero; cache-backed verified chain assembly, final
  flush/sealing, fixed setup/teardown orchestration and broader qualification are
  still required. No privileged host filesystem/BPF/device operations performed.

- Previous turn progressed in 8cc8daf. M4 storage now prepares verified scutes
  into owned data/tree inputs. Aligned uncompressed scutes retain the cache inode
  used for hashing; only verification metadata is generated in the RAM workspace.
  Compressed/partial-block scutes use bounded decoded/padded scratch as required
  by the block format. Root mismatch, changed length, decoder errors and I/O
  failures reject preparation. Scratch cleanup is anonymous-file RAII; no writable
  file handles escape PreparedScute. ScuteExtent::root delegates to the same
  streaming formatter with its existing discard sink; the new format method
  retains exact-extent/EOF/error checks and emits the tree for storage assembly.
- v89 passes three preparation cases, same-length corruption rejection, cache
  pathname replacement without changing the retained inode, and the normal suite.
  Independent host hash calculations verify emitted tree blocks and a short
  destination rejects write failure. Full root tests (28 compile-fail), root and
  fixture Clippy, formatting and Python checks pass. Synthetic fixture source and
  bytes, VM logs, source snapshots and artifact hashes are preserved. Privileged
  operations remained inside disposable QEMU.
- Focused review checked bounded decoding, padding/root equivalence, retained
  inode authority, no blanket cache copy, scratch cleanup and unchanged existing
  root-verification behavior. Preparation does not enforce future reads: owned
  loop/verity/read-only snapshot assembly and corruption-through-kernel tests are
  next. The cache inode may still mutate; dm-verity remains mandatory. No M4
  adherence pass or full sandbox qualification is claimed; M4 remains zero.

- Previous turn progressed in 8ad2658. M4 now consumes PreparedScute into an
  owned read-only VerityMapping. Both loop bindings use atomic read-only/autoclear
  configuration and one allocation attempt. Target parameters come from the
  authenticated extent; the generated header is not reparsed as authority. Pinned
  devmap builder defaults reject corruption/I/O errors, verify zero blocks and
  do not enable check-at-most-once. No command obtains raw backing/control FDs.
- Cleanup-only error variants retain exactly the mapping/loop/backing resources
  still live. Explicit close removes the mapping first, then confirms loop
  unbinding using status; busy removal can retry with all backing retained. Drop
  requests deferred mapping removal, closes autoclear loops and lets kernel
  references retain backing; this is not a teardown-success receipt. Two negative
  compilation cases prohibit reusing consumed preparation and dropping workspace
  before its mapping. Partial setup fault injection and combined primary/cleanup
  error reporting remain part of broader lifecycle integration.
- Preserved the initial E0106 compiler error and unbuilt source. v90 preserves a
  failed fixture assumption that opening a read-only block device for writing
  must fail. Corrected the test to attempt the actual write. v91 passes rejected
  writes, positive reads for aligned/raw-partial/compressed data, busy mapping
  removal followed by retry, and EIO for cache corruption after activation before
  the first read. Existing normal suite passes. Root tests (30 compile-fail),
  root/fixture Clippy, formatting/Python checks pass. Source/raw logs/input blobs
  and complete artifact hashes verified; no privileged host device operations.
- Focused review checked consuming transitions, retained dependencies and field
  drop order, immutable target authority, no unchecked kernel reads, no polling
  sleeps and cleanup phase ownership. This is not a milestone adherence pass.
  Read-only snapshot assembly over dm-zero, verified snapshot-header parsing,
  full chain composition, setup/teardown bounds and full fault qualification remain
  outstanding. M4 remains at zero; no M5 or whole-sandbox completion claim.

- Previous turn progressed in 7d3c166. Added ReadOnlyChain as an aggregate owner
  over the format's 64GiB dm-zero, verified COW mappings and read-only snapshot
  layers. Source-grounded comparison with carapace confirms the shared snapshot
  decoder, 4096-byte whitelist and read-through-verity requirement. The existing
  carapace deferred-removal API does not expose this explicit teardown contract;
  assembly uses ecosystem devmap/lodown types with conglobate-owned resources.
  Reference source/spec and Git state are archived without modifying carapace.
- Mapping node opens reject symlinks and verify block-device identity against the
  owned mapping before reading. Snapshot headers are read only through dm-verity;
  invalidated/non-whitelisted headers fail. Kernel snapshot health is checked
  before each new layer enters the usable aggregate. Private error phases retain
  partially constructed stacks. Explicit and best-effort teardown order is top
  to base, with no recursive chain representation or depth-dependent call stack.
  Failed removal retains upper resources and all lower dependencies for retry.
- v92 passes initial two-scute reads, holes, busy close/retry, and invalid second
  header cleanup. Review added the explicit kernel health gate and strengthened
  overlapping-layer coverage. v93 passes upper override, lower inherited block,
  zero hole, rejected writes, busy cleanup retry, and invalid upper header rejection
  after the lower layer was assembled. Mapping listings return to their baseline
  after explicit cleanup in both cases. Existing normal suite passes. Root tests
  (32 compile-fail), root/fixture Clippy, formatting/Python checks pass. Verified
  artifacts, exact sources and intermediate evidence retained; no privileged host
  device/mount operations. New lifetime tests separately identified after boot.
- Focused review checked aggregate ownership, ordered Drop/close, authenticated
  header provenance, retained partial-stack authority and exposed API limits.
  This is not an M4 adherence pass. Writable snapshot integration over assembled
  chains, mounted-volume ownership, combined primary/cleanup error reporting,
  bounded setup/teardown and broader failure/crash qualification remain required.
  M4 remains zero; no filesystem, M5 bootstrap, or full-build completion claim.

- Previous turn progressed in 61cdac3. SnapshotOrigin is now an immutable borrowed
  authority obtained from a read-only mapping or an assembled ReadOnlyChain. The
  conversion borrows the aggregate's internal top mapping without exposing it or
  permitting individual layer removal. SnapshotMapping and its failure owner both
  retain that borrow; loop backing remains consumed. Existing dm-zero call sites
  continue through the same conversion. No arbitrary device-ID origin constructor,
  runtime origin flag, or public raw handle was added.
- v94 passes a writable snapshot over the verified two-scute chain. Reads inherit
  the origin; writes replace one inherited block and fill a prior hole without
  changing the original view. A busy snapshot close retains ownership until the
  writer closes. Detach/reattach and reactivation preserve new bytes plus untouched
  inherited bytes. The original chain is reread and its cache content reverified;
  final explicit cleanup restores the original mapping listing. Existing normal
  suite passes. Root tests (34 compile-fail), root/fixture Clippy, formatting and
  Python checks pass; exact source/raw evidence/artifact hashes are preserved.
- Focused review checked borrowed aggregate lifetime, private field authority,
  consuming backing ownership, failure-path origin retention, and compatibility
  of the original zero-origin API. Two compile-fail cases prohibit closing the
  chain while either a snapshot or its setup-failure owner remains live. This is
  not sealing, mounted-volume integration, an M4 adherence pass, or M5 advancement.
  M4 remains zero. Next integrate owned mounted volumes with these storage owners;
  orchestration deadlines, primary/cleanup reporting and wider fault coverage
  remain required. No privileged host operations or sleep synchronization added.

- Previous turn progressed in 81ea79b. Added sealed Volume<Unmounted>/Volume<Mounted>
  markers with associated resource payloads. The owner consumes SnapshotMapping,
  borrows the private mount scope, and retains storage through mount/unmount
  failures. ext4 setup uses fsopen/fsconfig/fsmount with a pinned source-device FD,
  fixed nodev/nosuid flags, and an empty generated directory in a trusted tmpfs
  parent. There is no recipe-supplied mount command/options or raw root accessor.
- Mounted views borrow Volume; consuming unmount is unavailable while a view is
  live. Explicit teardown checks syncfs, closes its root FD, then performs a normal
  unmount. Flush, busy-unmount and target-cleanup failures retain distinct private
  cleanup-only ownership. No unmounted value escapes those failures. Snapshot
  access is restored only after successful unmount/target cleanup. Unmounted is
  not a sealing receipt or proof that all external kernel consumers have ended.
  Point cleanup only removes empty directories, never recursively walks images.
  Drop may lazy-detach as best effort but cannot produce a success receipt.
- v95 passes unformatted-mount failure with retained storage and empty-target
  cleanup, ext4 mounting over owned snapshot, file writes, cloned view lifetime,
  EBUSY from an independent open-directory holder, cleanup retry, remount with
  persisted data, and final storage release. Formatting is fixture-only mke2fs;
  production typed formatting remains M5. Root tests (36 compile-fail), root and
  fixture Clippy, formatting/Python checks pass. Source/artifact hashes verified;
  exact booted source distinguished from later compile-fail documentation.
- Focused review checked field drop order, fallible operations before attachment,
  source-device identity, nonrecursive target cleanup, borrowed-view lifetime and
  recovery-state authority. No M4 adherence pass claimed; M4 remains zero. Next
  integrate owned volumes with command views and initial-namespace policy identity
  retention/handoff. Fixed setup/teardown bounds, combined error reporting and
  fuller fault/crash coverage remain required. All privileged tests stayed inside
  disposable QEMU; no host mount/device operations or synchronization sleeps.

- Previous turn progressed in 871f172. Integration review found that volumes also
  need supervisor ownership in the verified init namespace, where policy identity
  pins live; command views still need private mount namespaces. Added the typed
  init-namespace constructor using RecoveryDirectory's existing namespace proof,
  and Volume::pin creates the exact independent FilesystemPin without exposing a
  root FD. Mount/unmount/cleanup verify their owner namespace; namespace mismatch
  retains cleanup-only ownership. PrivateMountNamespace now pins its own identity
  and checks it before detached-view assignment/cloning.
- v96 preserves EINVAL from attempting open_tree(CLONE) on an attached root FD
  after entering another namespace. Local kernel may_copy_tree confirms that a
  foreign non-anonymous source cannot be cloned that way. Fixed the handoff:
  Volume::view clones nonrecursively in the owner's namespace and returns a
  borrowed VolumeView; consuming in_namespace assigns the detached tree to the
  private command scope. Its lifetime bound keeps the volume borrowed through
  later Mount states. No raw mount or namespace descriptor escapes these APIs.
- v97 passes both private and supervisor volume paths, identity pinning and
  namespace-rejection/retry checks. v98 additionally creates a mapped user
  namespace, idmaps/attaches the detached view inside a new private namespace,
  observes underlying UID 0 as mapped UID 100000, then releases views/pins and
  remounts successfully. Wrong-namespace pin/view/cleanup attempts fail without
  releasing their storage. Existing normal suite passes. Root tests (37 compile-
  fail), root/fixture Clippy, formatting/Python checks pass. Failed/intermediate
  runs, exact sources, kernel source excerpt, logs and artifact hashes retained.
- Focused review checked pre-transition cloning, namespace identity verification,
  borrowed view conversion, pin/device/inode identity and cleanup-only retries.
  This resolves a construction mismatch; it does not yet couple these volumes to
  a complete protected command Step or qualify supervisor-death behavior for the
  new path. That integration, fixed setup/teardown bounds, combined error reporting
  and broader faults remain required. M4 stays zero; no adherence advancement or
  whole-sandbox claim. Privileged work remained in disposable QEMU; no sleeps added.

### M4 retained namespace authority (v99)

- Previous turn checked the sleep objection without changing implementation; this
  turn resumes concrete M4 work. No sleep-based synchronization was introduced.
- NamespacePin now retains its original namespace descriptor. Recovery opens and
  verifies one descriptor and retains that same identity. UserNamespace::idmap
  applies the pinned mapping without exposing a raw descriptor or reopening a
  fixture pathname; descriptor duplication and mapping failures retain the mount.
- The volume fixture uses this API in both private and init mount namespace modes.
  v99 passes idmapped views, policy pins, busy unmount recovery and persistence.
  Root tests (including 37 compile-fail cases), root and fixture Clippy pass.
  Exact sources, logs and hash provenance are archived under evidence/v99.
- This is a prerequisite for descriptor-based helper setup, not that completed
  handoff: the protected-command fixture still uses /work/userns-path and
  /work/origin. Transfer over supervision and Step/Volume aggregate ownership,
  bounded setup/teardown and broader fault qualification remain outstanding.
  M4 has zero clean adherence passes; no milestone advancement is claimed.

### M4 supervised volume transfer (v100)

- Previous turn progressed in 9871c40. Registered supervision now transfers a
  detached VolumeView and the typed pinned user namespace over its existing
  seqpacket channel, with deadline-based poll, exact opcode/descriptor counts
  and CLOEXEC receipt. The helper validates object types, idmaps the detached
  mount and constructs the separate namespace-entry/device input without
  reopening the source volume or namespace by pathname. Admission remains a
  separate protocol operation; receiving a volume cannot authorize a command.
- v100 adds a dedicated registered helper to the owned init-namespace volume
  probe. It receives both descriptors, enters a private mount namespace, idmaps
  and attaches the received view, checks mapped ownership and publishes a
  readiness event. The parent retains its view through cancellation/reaping
  before continuing the existing unmount/recovery/remount checks. No command
  executes in this transport-only probe and no timing sleep coordinates it.
- Host tests include missing/extra opcode descriptors, CLOEXEC and descriptor
  order checks; full root tests (37 compile-fail), root/fixture Clippy, and VM
  suite pass. Exact sources, logs and hashes are archived under evidence/v100.
- Review boundary: send_volume_until is a transport primitive and does NOT
  enforce the aggregate volume lifetime. The enclosing Step must own/retain
  storage through descendant quiescence, including failed startup/cancellation.
  The full protected-command fixture still uses its historical source paths.
  Connect these owners and replace that path next; do not interpret the passing
  probe as full command integration. Setup/teardown deadlines, broader failures
  and qualification also remain open. M4 stays at zero clean adherence passes.

### M4 step-owned mounted volume (v101–v102)

- Previous turn progressed in 05748d6. Step now carries sealed StepStorage through
  every success/failure transition. Ready.with_volume consumes the mounted owner;
  there is no active-state accessor or replacement operation. The retained owner
  deliberately survives unfinished-step destruction until dedicated-VM disposal.
  Successful drain, record cleanup and policy disable return StepCompletion,
  which yields the volume alongside disabled policy and diagnostic child status.
- Step.send_volume_until derives the namespace from its bound policy and checks
  the volume root against an owned filesystem pin (device and inode). The raw
  supervision send is now crate-private. This prevents callers bypassing the
  storage-owning entrypoint for the new volume-transfer protocol.
- VM v101/v102 exercises owned volume transfer with policy preparation, explicit
  admission, event readiness, cancellation, reaping, descendant drain, policy
  disable/cleanup and volume recovery. Subsequent idmap/unmount/busy recovery and
  persisted remount checks still pass. This helper runs no image commands; the
  older full command fixture still needs conversion from its historical paths.
- v101 preserves successful runtime evidence plus Clippy findings: complex result
  tuple and an explicit drop in the retention test. v102 uses StepCompletion and
  lexical test scope; host tests including 39 compile-fail checks, both Clippy
  checks and VM suite pass. Compile-fail checks prohibit reusing the moved volume
  or disabling a running volume-owning step; retention test distinguishes ordinary
  unfinished scope exit from explicit release. All logs/sources/hashes retained.
- Review limits: no dedicated fault-injection coverage yet for owned-volume
  startup/teardown failure and supervisor death; a successful helper probe does
  not prove those paths. Bootstrap needs a sealed two-volume storage aggregate.
  Full command integration, bounded setup/teardown orchestration and remaining M4
  fault/qualification work stay open. Zero clean adherence passes; no advancement.

### M4 native commands on step-owned volumes (v103–v104)

- Previous turn progressed in eba05e3. The owned-volume helper now executes real
  image commands through transferred root/namespace descriptors, private runtime
  mounts, namespace init, command privileges and syscall restrictions. Its script
  is shared with the earlier native-command fixture, including SELinux allow/deny
  cases, absence of supervisor/control paths, capability checks and child adoption.
  Fixture-only population copies the existing test image into the mounted volume;
  this is not production COPY or M5 formatting implementation.
- v103 passes success, execution-timeout cancellation and shell failure (37).
  v104 adds cancellation before admission and an expired admission deadline.
  The latter cases verify no command-started or labelled file exists. Every case
  drains descendants, disables policy, recovers the volume and runs subsequent
  busy-unmount/retry/remount checks. For executed commands, native on-disk UID 0
  and exact security.selinux bytes are verified from the supervisor root view.
- The timeout waits on the helper pidfd with a finite deadline after observing
  command readiness; no scheduling sleep is used. Completion marker publication
  uses rename so observation cannot see a partially written status. Shell exit
  37 is explicitly checked by helper and supervisor. All five v104 markers and
  fixture exit 0 are present. Four host readiness tests and fixture Clippy pass;
  production sources are unchanged from the prior tested commit. Both complete
  VM runs, exact sources and hashes are archived and verified.
- Still open: owned-volume supervisor-death/partial-setup fault qualification,
  fixed setup/teardown orchestration around blocking operations, combined cleanup
  diagnostics and production build orchestration. Historical path-based fixtures
  remain for their independent failure/death coverage; the newly exercised path
  obtains its command root only from the owned-volume transfer. No M4 adherence
  pass claimed; streak remains zero and later milestones remain unadvanced.

### M4 owned-volume setup faults and supervisor death (v105–v106)

- Previous turn progressed in 7d39125. v105 adds two benign fixture fault points:
  error return after detached-root preparation, and helper process exit after
  attachment without Rust destructors. Both fail admission before any command,
  retain Step cleanup ownership, drain the domain and return the mounted volume.
  No labelled/command-started output exists, and subsequent unmount/remount checks
  succeed. These are particular setup boundaries, not exhaustive syscall faults.
- v106 moves the independent-controller death exercise to the owned-volume
  command path. After readiness the guest controller sends SIGKILL to conglobate,
  confirms a live command domain, locates the production filesystem pin and
  unmounts the original volume mount. The still-contained command changes and
  reads its allowed SELinux label while an unassigned filesystem remains denied.
  The controller observes the published check marker and shuts the VM down.
  The log records all owned-volume/policy/pin markers and kernel Power down.
  No guardian/recovery service or successful build receipt is introduced.
- Normal suite v105 and owned-volume death suite v106 pass. Four host readiness
  tests and fixture Clippy pass; production sources are unchanged. Exact
  sources, failure-injection behavior, serial logs and hashes are preserved and
  verified. Earlier path-based death evidence remains recoverable in its archives.
- This closes the specific new-path death and two partial-setup evidence gaps,
  not all M4 qualification. Remaining work includes fixed setup/teardown
  orchestration around blocking operations, combined primary/cleanup diagnostics,
  further fault review, and production stage orchestration. M4 has zero clean
  adherence passes; no milestone advancement or whole-sandbox claim.

### M4 cleanup diagnostic history (v107–v108)

- Previous turn progressed in f79cc20. MappingFailure and VolumeUnmountFailure
  now retain FailureDiagnostics: the original io::Error plus ordered failed
  cleanup attempts. SnapshotMappingFailure exposes the underlying diagnostics.
  Consuming retry transitions preserve history and cleanup-only resource owners;
  source_error remains the most recent error for existing callers. The explicit
  diagnostics accessor retains original OS codes and concrete error payloads.
- Unit coverage checks flattening, original errno and retry ordering. VM probes
  retry a busy mapping before releasing its holder, and retry unmount twice from
  the wrong namespace after an original EBUSY. They assert preserved original
  causes and cleanup errors, then restore conditions and complete cleanup.
- v107 passes runtime/tests but Clippy finds the larger unmount error layout.
  v108 boxes the diagnostic history, preserving the original compact failure
  layout without boxing each retry or suppressing the lint. Root tests including
  39 compile-fail cases, both Clippy checks and normal VM suite pass. Both exact
  implementations and evidence, including the Clippy finding, are archived with
  verified hashes. No timing retries or sleeps were added.
- Coverage is intentionally specific: outer VerityFailure/ReadOnlyChainFailure
  still replace errors during their cleanup transitions, and other lifecycle
  failure reporting must be reviewed. Propagate history there next; do not claim
  combined diagnostics complete. Fixed deadline orchestration and remaining M4
  qualification/adherence also remain open. Clean-pass streak stays zero.

### M4 nested storage cleanup diagnostics (v109)

- Previous turn progressed in a7cf36f. VerityFailure and ReadOnlyChainFailure now
  preserve their operation and ordered cleanup-attempt errors across retries.
  Storage diagnostic histories share immutable io::Error payloads at ownership
  boundaries; latest_only observes the concrete current child error without
  copying formatted history or losing OS codes. Each outer retry appends exactly
  that attempt's error; resource owners remain in the existing cleanup-only
  phases. Read-only history access and consuming transitions remain the API.
- Unit coverage verifies the same concrete payload survives a boundary and no
  prior history is replayed. VM v109 repeats a busy verity removal twice and a
  busy chain removal once, checks exact errno values and history lengths, then
  releases readers and successfully closes all mappings. It also runs the prior
  volume and native-command cases. Root tests (39 compile-fail), both Clippy
  checks and VM suite pass; source/log/artifact hashes are archived and verified.
- This covers the storage wrappers identified in the preceding finding. It does
  not establish complete lifecycle diagnostic handling: the eventual build/step
  orchestration must retain a command's primary error separately from teardown
  failures, and setup/teardown still need fixed internal deadline orchestration.
  SANDBOX-DESIGN 10.2 bounds waiting/acceptance rather than promising immediate
  reaping of uninterruptible kernel I/O. Follow that distinction in the next
  implementation. M4 remains zero clean adherence passes, with review and other
  qualification requirements outstanding.

### M4 fixed-budget step teardown (v110–v111)

- Previous turn progressed in 3d45779. Step::teardown now coordinates cancellation,
  helper reaping, descendant drain, record cleanup and policy disablement with a
  single 30-second internal budget. Ready, Running and Cancelling entrypoints
  consume their owners. Internal recovery variants contain the exact typestate;
  failure exposes diagnostics and retry only, with no deadline-extension input.
  Retry retains both phase and original cutoff and appends failed-attempt errors.
- The coordinator checks expiry before every transition and before accepting a
  completion, including after synchronous record/policy operations. It performs
  no polling retry loops or sleeps. This bounds waiting/acceptance, not the duration
  of a kernel syscall already blocked in uninterruptible I/O. Expiry fails the
  build and preserves outstanding guards/storage under the VM-disposal contract.
- Owned-volume success, timeout, shell failure, pre-admission cancellation,
  admission timeout and both setup fault cases use the coordinator in v110/v111.
  Normal VM suite passes. v110 preserves a Clippy large-transition-error finding;
  v111 boxes that internal carrier. Host tests (40 compile-fail), both Clippy
  checks and VM pass. A host check tests immediate expiry without sleeping;
  compile-fail coverage prevents passing a replacement deadline to retry.
- Boundary remains incomplete: this coordinator stops at policy disablement.
  Pin release and filesystem flush/unmount/finalization still need to carry the
  same overall teardown budget, rather than starting fresh budgets. Setup needs
  its own fixed budget, and primary command outcome must survive cleanup errors.
  Late kernel-return/failure coverage also needs review. M4 remains zero clean
  adherence passes; this is not full lifecycle-deadline qualification.

### M4 complete single-volume teardown budget (v112–v113)

- Previous turn progressed in cbd257d. The fixed teardown cutoff now survives
  policy disablement, disabled-policy cleanup, filesystem pin release, namespace
  pin release, volume flush and clean unmount. TeardownCompletion retains the
  deadline and mounted storage privately. Its unmount conversion returns only a
  FinalizedVolume containing Unmounted storage. A successful return is not a
  sealed-image receipt or proof of command success.
- Cleanup variants retain partially removed policy resources, individual mounted
  pins versus unmounted targets, and the remaining pins. Filesystem pins release
  before the namespace pin. Each operation is checked against the same cutoff;
  late-returning operations cannot produce accepted completion. Finalization
  failures retain exact volume cleanup state and deadline; retries cannot extend
  it. No timing loops, watchdog process or forced unmount receipt is introduced.
- v112 exercises all owned-volume cases through the complete budget, forcing a
  busy unmount then releasing its holder and retrying. v113 additionally blocks
  policy directory cleanup, verifies identity owners remain, removes the blocker
  and retries. The fixture recreates separate identities only for its subsequent
  independent view tests after production finalization has released the originals.
- VM suites pass. Root tests (42 compile-fail), both Clippy checks and formatting
  pass; compile-time tests prohibit extracting mounted storage and supplying a
  replacement finalization cutoff. Both exact source/evidence sets are retained
  with verified hashes. Deadline expiry after actual slow kernel operations and
  more pin-failure boundaries still need review; this is not exhaustive faults.
- Remaining M4 work includes setup budget orchestration, preserving primary
  command outcome across cleanup failures, production orchestration and adherence
  review. Bootstrap's two-volume aggregate is not implemented by this single-
  volume result. Streak remains zero; no milestone advancement or whole-build
  qualification is claimed.

### M4 execution-boundary finding and restricted command (v114)

- Previous turn progressed in f98b7f8. Setup-clock source review found that the
  fixture's outer helper acknowledges admission before the inner helper installs
  runtime mounts, pivots root, applies capabilities and installs seccomp. Therefore
  outer admission cannot yet mark the command-only execution-clock boundary.
  Adding a fixed timeout around that acknowledgment would preserve the wrong
  semantics. This is a concrete outstanding M4 finding, not a clean pass.
- EnteredRoot::restrict now consumes root state, installs null stdin and final
  restrictions, and returns a thread-affine RestrictedCommand without running
  image code. RestrictedCommand::execute performs the final descriptor audit
  immediately before spawning namespace init's image shell. It consumes the
  command; no public mutation or descriptor access is exposed. The old execute
  convenience method delegates to the same transitions for compatibility.
- v114 runs the normal VM cases through the explicit restricted-command boundary.
  A fixture-only extra /dev/null descriptor is opened after restriction; final
  audit rejects it and verifies no command-started/labelled output exists. Normal
  commands still pass all restrictions/label checks. Host tests (43 compile-fail),
  both Clippy checks, formatting and VM pass. Exact source/evidence hashes are
  archived and verified.
- Next: a trusted leaf-ready/start handshake at this boundary, closing all startup
  descriptors before the final audit, then one setup cutoff and a separate finite
  execution timeout. Namespace-init validation/spawn overhead must be considered
  explicitly when defining that boundary. The current fixture still acknowledges
  admission early; do not claim setup/execution timing is resolved. Primary command
  outcomes versus cleanup diagnostics and M4 review also remain open; streak zero.

### M4 restricted-leaf start gate (v115–v116)

- Previous turn progressed in 66d2a4c. ExecutionGate has sealed Waiting/Ready
  states: the parent cannot release execution before receiving readiness. The
  restricted leaf sends readiness, waits for explicit start and closes its gate
  before the final descriptor audit and image spawn. Messages are exact versioned
  CBOR records over nonblocking seqpacket sockets with poll deadlines; unexpected
  ancillary rights, wrong-phase packets, truncation, EOF and expiry fail closed.
  Parent cancellation shuts its endpoint down; no sleep establishes readiness.
- GatedCommandSetup and GatedRootSetup transfer namespace/device/gate ownership
  through the trusted setup chain using bounded descriptor packets. The owned-
  volume fixture now launches this chain before awaiting upstream admission.
  Only after the fully restricted leaf reports ready does the outer helper accept
  admission and release start. Startup handles are absent at the final audit.
- v115 normal VM suite and v116 supervisor-death suite pass. Existing command,
  label, descriptor-rejection and cleanup cases run through the gated path.
  Host tests cover explicit release, cancellation after readiness, expiry, wrong
  phase, malformed packets, unexpected rights and descriptor closure on rejection;
  44 compile-fail cases pass. Both final Clippy checks pass. Initial rustix API
  compile errors and source are retained under gate-compile-v1. v116 preserves a
  redundant-reference Clippy finding; its post-run borrow-only correction and
  final host checks are separately recorded from exact booted source. All hashes
  verified; no full-sandbox or adherence claim.
- Still needed: one production fixed setup budget across root/policy/helper
  preparation (fixtures currently supply deadlines), the command execution clock
  and primary outcome carried through teardown failures. The authorized start
  boundary still includes final descriptor audit and process spawn/init validation;
  document/account for that overhead rather than calling an earlier helper ACK
  actual shell execution. Older ungated fixtures/API entrypoints remain for
  independent coverage and require review before final orchestration. M4 streak
  remains zero; later milestones remain unadvanced.

### M4 setup budget and supervisor-visible readiness (v117)

- The prior turn inspected synchronization without implementation progress. This
  turn adds SetupBudget (fixed 30 seconds), PreparedStep and RunningCommand. The
  trusted helper reports Prepared only after its restricted leaf reports ready.
  The supervisor consumes readiness before admission. Execution time begins before
  sending admission; acknowledgment latency, final FD audit and spawn count toward
  the finite command timeout. Late admission retains the running owner for cleanup.
- The owned-volume fixture creates its budget before preparation and uses the same
  cutoff for helper launch, volume transfer and readiness/admission. This is not
  yet complete production orchestration: intermediate acquisitions still need
  budget checks and potentially blocking operations need bounded execution.
- v117 normal disposable-VM suite passes, including timeout, setup failures,
  descriptor rejection and cleanup retry. Host tests cover readiness ordering,
  immediate expiry, wrong-phase packets and peer loss, with no sleeps. All host
  tests and 44 compile-fail tests pass; both Clippy checks pass. The initial large
  error-variant finding and source are preserved in setup-budget-clippy-v1; boxing
  its retained RunningCommand fixes it without losing cleanup authority.
- v117/source preserves exact booted source. Added host readiness tests are saved
  separately as post-run source. No adherence pass claimed; M4 streak remains zero.
  Production orchestration, primary command outcomes through cleanup failures and
  remaining M4 review are still outstanding. Later milestones remain pending.

### M4 primary command outcome and cleanup association (v118–v120)

- Previous turn progressed in c6688ef. The owned-volume fixture previously read
  command status from a marker file while teardown returned the outer trusted
  helper's exit status. These must not be conflated. Completion now travels on
  the private supervision channel as exact CBOR [1,7,bstr(four-byte big-endian
  Linux wait status)], with no ancillary descriptors. The helper reports before
  its own mount cleanup. The fixture no longer publishes a status marker file.
- RunningCommand::observe consumes the running owner and retains it alongside
  Exited or ObservationFailed, using the original execution cutoff and rejecting
  late observation. CommandResult consuming cleanup transitions retain that
  primary outcome even when cleanup returns a failure/retry owner. This is a
  diagnostic association, not a success receipt or proof of finalized storage.
- v118 passes ordinary completion/timeout/cleanup retry. v119 failed to compile
  a fixture &String pattern; exact source/logs are preserved, with no stale VM
  run. v120 corrects the pattern and combines shell exit 37 with policy cleanup
  failure and retry, proving 37 survives both transitions. Host tests preserve
  nonzero status and timeout through cleanup, validate exit/signal transport and
  reject nonterminal/reserved statuses, malformed lengths and extra descriptors.
  Host tests, 44 compile-fail cases, both Clippy checks and v120 VM pass. The
  initial Clippy items-after-tests finding is preserved in v118 and fixed by
  moving the implementation before tests. Source/evidence hashes are verified.
- Namespace init still normalizes shell signal termination to 128+signal. The
  completion transport preserves the reported namespace-init result; it does not
  recover original shell signal identity. Helper failure and command failure
  remain distinct observations. Finalization reports must continue carrying the
  primary outcome in the production orchestrator; fixture extraction currently
  separates them before its independent busy-unmount tests. Intermediate setup
  acquisition checks and full orchestrator integration remain outstanding.
  M4 adherence streak remains zero; no later milestone is advanced.

### M4 outcome-preserving storage finalization (v121)

- Previous turn progressed in fa0fac1. CommandResult::transpose now branches on
  cleanup success/failure while retaining the primary outcome in both branches.
  The mounted-volume completion has a consuming unmount method returning either
  an outcome-associated FinalizedVolume or an outcome-associated failure owner.
  Its retry retains the existing finalization deadline and original outcome.
  FinalizedVolume documentation now explicitly identifies its separate status as
  the outer helper's status rather than the recipe command outcome.
- The fixture no longer separates command outcome from storage before unmount.
  A fixture-only enum distinguishes aborted setup (no command observation) from
  observed commands. A benign open directory forces unmount EBUSY. Both before
  retry and after finalization it checks the exact primary outcome: zero, exit
  37, or timeout. The policy-cleanup mode exercises exit 37 through policy cleanup
  failure/retry followed by busy-unmount failure/retry in the same build step.
- v121 normal disposable-VM suite passes and requires the new finalization marker.
  Host suite including 44 compile-fail tests, both Clippy checks and formatting
  pass. Exact source/logs and hashes are preserved and verified. No synchronization
  sleeps added. M4 adherence streak remains zero: these are tests, not review
  passes. Full production acquisition/orchestration and remaining M4 design/error
  path review are still outstanding; later milestones remain unadvanced.

### M4 focused integration review after v121

- Previous turn progressed in 78eaf96. Reviewed actual production module/ownership
  boundaries against SANDBOX-DESIGN sections 7.6 and 10, ARCHITECTURE and the
  owned-volume fixture. Findings I1–I7 and source evidence are recorded in
  M4-INTEGRATION.md. This is a focused review with findings, not a full adherence
  pass; clean streak remains zero.
- The next implementation must extract the real helper/orchestrator path rather
  than add more independent receipt wrappers. In particular, fixture helpers
  currently use fresh five-/thirty-second cutoffs and independent thirty-second
  child waits. Copying those would violate shared setup timing and truncate
  recipes whose allowed execution exceeds thirty seconds. Current short VM tests
  do not establish support for the five-minute default or longer recipes.
- Preparation before Step::new still lacks an aggregate error/cleanup owner;
  fixture string conversions are not a production recovery implementation. Typed
  acquisition ownership, immutable helper inputs and supervisor-owned timing are
  the next work. No new guardian is authorized or needed. No code or validation
  artifacts removed; no passing implementation claim for these open findings.

### M4 production namespace/root helper extraction (v122–v123)

- Previous turn progressed in b419e25 with source-grounded integration findings.
  GatedCommandSetup::execute now owns namespace entry, root-input transfer,
  trusted child launch and wait. The process wait used here is crate-private and
  has no independent execution timeout: this helper is already inside the
  supervisor-owned containment domain. Supervisor deadline/cancellation remain
  required. Wait failures retain the child owner. No timer loop or sleep added.
- GatedRootSetup::restrict now owns private mount namespace/root/runtime setup,
  privilege/seccomp restriction and start endpoint retention. Its immutable
  GatedRestrictedCommand is consumed at start. Errors are exit-only in trusted
  helpers; no supervisor restoration or new guardian is introduced. The fixture
  calls production methods and keeps benign extra-descriptor injection separate.
- v122 normal VM and v123 supervisor-death VM pass. Timeout/cgroup cancellation,
  label/descriptor checks and outcome-preserving cleanup still pass through the
  extracted path. Host tests (44 compile-fail), both Clippy checks and formatting
  pass; exact source/logs and all evidence hashes are preserved and verified.
- I1/I3 are partly resolved, not closed. The outer receiver and binary dispatch
  are not extracted, the receiver still has an independent thirty-second wait,
  and setup cutoffs still need propagation. Preparation aggregate/error ownership
  also remains open. M4-INTEGRATION.md records these limits. M4 streak zero;
  tests are not adherence passes, and later milestones remain pending.

### M4 production receiver and long-command regression (v124–v125)

- Previous turn progressed in a978af6. CommandReceiver now consumes the registered
  channel, attached mount and namespace setup, owning gate creation, launch,
  restricted readiness, admission, start, command-helper wait and completion
  reporting. Failure variants retain attached/in-use mount and child wait owners.
  Root wait uses the supervisor-controlled wait with no independent command
  cutoff. Host setup errors remain exit-only; no guardian is introduced.
- Found and fixed an additional timing issue: the fixture reported completion
  using its old setup deadline. Production receiver status delivery instead uses
  a fixed internal 30-second communication bound. The parent still enforces the
  original recipe execution cutoff; report delivery cannot extend that cutoff.
- v124 normal VM passes a new duration regression: assigned-root FIFO blocks the
  command; an atomic readiness marker establishes entry, a pidfd wait verifies
  the helper does not exit over 31 seconds, then an explicit FIFO write releases
  it. The command completes successfully under a 45-second recipe limit, through
  finalization. No synchronization sleep is used. v125 supervisor-death VM also
  passes, including the duration regression prefix and external VM poweroff.
- Host suite (44 compile-fail tests), both Clippy checks and formatting pass.
  Initial thiserror non-static-source compilation failure is preserved in
  receiver-compile-v1. Exact source/logs and evidence hashes verified. No full
  qualification/adherence claim; M4 clean streak remains zero.
- I3's independent execution limits are removed for the extracted path. Shared
  cross-process setup cutoff (I2), initial input/mount/device preparation, binary
  dispatch, parent acquisition ownership and successful receipt gating remain
  open. M4-INTEGRATION.md records the partial resolution; M5–M8 remain pending.

### M4 shared setup cutoff transport (v126)

- Previous turn progressed in 88a486c. SetupBudget exports a SetupCutoff using
  CLOCK_MONOTONIC through rustix's safe time feature. Deterministic versioned CBOR
  [1, nanoseconds], hex-encoded for trusted argv, is forwarded unchanged through
  receiver, namespace helper and restricted root entry. No Rust Instant internals
  are serialized and no remaining duration is restarted on receipt. Clock sample
  order conservatively shortens rather than extends the original cutoff.
- Namespace entry checks the cutoff before setup and child launch. Root setup
  checks before preparation and after restriction; the restricted command retains
  the cutoff and checks it at start. Receiver uses it for readiness/admission;
  parent namespace creation also uses the original SetupBudget deadline. These
  helpers share the VM time namespace; cross-VM/time-namespace transfer is outside
  this private launch contract. Completion keeps its distinct communication bound.
- Host tests prove repeated forwarding preserves the exact value and does not
  extend the original Instant, and reject expiry, malformed/trailing/noncanonical
  encodings. v126 normal VM passes through all helper stages, including the
  31-second FIFO-held command under a 45-second execution budget, cancellation,
  setup failure, label/descriptor and finalization cases. Host suite with 44
  compile-fail cases, both Clippy checks and formatting pass. Evidence hashes
  and exact booted source are preserved and verified.
- I2's resetting helper timers are resolved for the exercised path. This does
  not prove every potentially blocking acquisition is bounded or has a retained
  aggregate failure owner. Those I4 requirements, binary dispatch, launch-input
  integration and success-receipt gating remain open. M4 streak remains zero;
  no full adherence pass or later milestone advancement is claimed.

### M4 activation-handle acquisition before helper launch (v127–v129)

- Previous turn progressed in 678de92. Source review found Step::new duplicated
  the activation map handle after contained-helper launch. Bound policy binding
  now acquires that handle and retains it with policy/identity ownership. Its
  existing BindFailure returns all owners if acquisition fails. Step::new is
  now an infallible transfer; the unnecessary post-launch PrepareFailure state
  and its call-site error handling are removed. No new recovery process added.
- Policy activation/disable use the already-owned handle. Cleanup retains it on
  error and drops it before returning identities after successful policy removal.
  Active steps retain the whole policy under their existing explicit teardown
  guard. This preserves the policy-before-identity cleanup ordering.
- v127 was compile-only: an older fixture caller still expected Result from the
  constructor. v128 booted and rejected a test assertion that incorrectly assumed
  descriptor exhaustion occurred at map-handle acquisition. It actually occurs
  earlier during filesystem identity verification. Both failures are preserved.
  v129 checks the actual EMFILE, retained policy/identity ownership and successful
  binding retry after restoring the limit. It does not claim isolated coverage
  of the exact map-handle duplication failure branch. Ordinary activation and
  policy-cleanup failure/retry exercise the newly retained handle.
- v129 normal VM suite passes including long-command and outcome/finalization
  regressions. Host tests (44 compile-fail), both final Clippy checks, formatting
  and evidence hashes pass. I4 is reduced, not closed: earlier acquisition and
  helper-launch failures still need a preparation aggregate. M4 streak remains
  zero; no clean adherence pass or later milestone advancement is claimed.

### M4 production helper dispatcher (v130–v131)

- Added HelperProgram/HelperInvocation with bounded canonical launch inputs,
  explicit receiver identities/mount parent and immutable absolute setup cutoff.
  Dispatch always exits after namespace/root changes. Ordinary VM cases now use
  these production roles; deliberate fault injection stays in the fixture.
- v130 normal VM suite passes, exercising labels, timeout, nonzero exit,
  cleanup/finalization retry and execution beyond the setup deadline.
- Host tests cover role round trips, unchanged cutoff, non-UTF8 mount path,
  malformed/trailing/oversized inputs and invalid receiver identities/paths.
  Root and fixture Clippy pass with warnings denied.
- This does not provide the public build binary or the aggregate parent
  preparation/rollback owner. Successful-step receipt gating remains open.
  M4 stays at zero clean adherence passes; M5–M8 remain pending.

- v131 supervisor-death suite passes through the production dispatcher; the
  independent controller verifies retained live-domain policy/storage and
  shuts down the VM. Source, host checks and artifact hashes are archived.

### M4 aggregate helper-launch rollback (v132 onward)

- CommandPreparation consumes bound policy/identities, mounted volume, recorded
  domain and child reaper before helper launch. On success it transfers them
  into Step<Ready>; on failure its cleanup-only owner retains every acquired
  resource through cancellation, child/descendant reaping, record/policy/pin
  removal and storage finalization.
- Rollback has a fixed thirty-second deadline, including retries. Original
  launch error and cleanup attempts remain separate; a rolled-back setup failure
  cannot become successful command evidence. Compile-fail tests reject live
  storage extraction and caller-supplied replacement cleanup deadlines.
- v132 normal VM suite passes through this launch owner, including executable
  absence and identity-exchange failure combined with policy cleanup and EBUSY
  unmount failures/retries. Host tests and Clippy pass. The initial compile
  failure is preserved separately; no validation history was removed.
- Earlier acquisition (namespace, mounts, policy construction/binding, domain
  creation) is still outside this owner. Full I4 integration, readiness error
  cleanup, success receipts and three adherence reviews remain outstanding.
  M4 stays at zero clean passes; M5–M8 are pending.

- v133 adds registration-rejection rollback coverage and passes the normal
  suite. v134 passes supervisor-death regression through the aggregate launch
  owner. All three startup failure variants retain their primary error across
  policy and unmount cleanup retries. Evidence/source hashes are verified.

### M4 integrated command execution and acceptance (v135)

- Step::execute now owns volume transfer, leaf readiness, admission under the
  shared setup cutoff, command observation under the recipe timeout, teardown
  and storage finalization. Failed setup/admission is explicitly cleaned up;
  cleanup failure retains the exact owner and primary outcome for retry.
- ExecutedVolume is privately constructed only when the command succeeds and
  its own storage finalization succeeds. It is not a layer/image seal.
  Public generic diagnostic mapping cannot manufacture this receipt.
- v135 passes the normal VM suite plus integrated exit 0, exit 37, timeout and
  helper-setup failure cases. Every integrated case forces EBUSY unmount, rejects
  early storage extraction, then retries. Only exit 0 is accepted; all other
  primary failures survive successful cleanup. Host tests (47 compile-fail),
  root/fixture Clippy, formatting and evidence hashes pass.
- Full earlier acquisition, public build entrypoint, API visibility review and
  the three M4 adherence passes remain outstanding. No milestone advancement
  is claimed. The next integration boundary is namespace/mount/policy/domain
  acquisition before CommandPreparation.

### M4 production policy acquisition (v136 onward)

- Added identity-owning PolicyInstallation for embedded-object load, disabled
  attachment/pinning and identity binding. It checks the original setup cutoff
  around acquisitions and rejects a supervisor outside the initial user
  namespace. Trusted storage construction still establishes superblock owner
  provenance; stat alone cannot prove it.
- CommandPreparation::install now retains installation failures together with
  the domain, reaper and mounted volume. Rollback removes disabled policy before
  identity pins and storage; partial attachments and late binding have typed
  cleanup owners rather than string-only propagation.
- v136 normal suite passes with ordinary commands using production installation.
  A missing policy parent preserves ENOENT through domain/pin cleanup and a
  forced EBUSY unmount/retry. Host tests, root/fixture Clippy and source/artifact
  hashes pass. Initial large-enum Clippy failure is preserved.
- This moves the policy acquisition boundary into production. Namespace/mount/
  filesystem-pin acquisition and domain creation/publication remain outside it.
  Partial attachment/late-binding aggregate faults still need coverage review.
  M4 clean streak stays zero; no milestone advancement is claimed.

- v137 supervisor-death regression passes through production policy installation.
  Its source/artifact hashes and independent-controller shutdown evidence are
  preserved; this does not constitute a full adherence or qualification pass.
