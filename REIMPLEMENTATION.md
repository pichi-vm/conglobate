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
| 3. Inputs and planning | New safe Rust crate; required context-relative recipe/lock paths; exact committed context and local verified image references; one recipe mapping, immutable plan, complete stage validation/inheritance; meaningful positive/negative tests. | Complete | 3 |
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
