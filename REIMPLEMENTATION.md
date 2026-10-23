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

### Current revision: environment-owned labeling exception

The user's registration-removal direction supersedes the scoped-policy wording
in the stored goal and historical passes. DESIGN.md and SANDBOX-DESIGN.txt now
require a fixed environment-loaded exact-name exception with no conglobate
policy installation, registrations or revocation. Preserve cgroup containment,
namespace ownership for idmapped mounts, descendant drain and storage cleanup.
The passing global-label-v2 probe is mechanism evidence only.

Reopen M2 and M4–M6 at zero clean passes for affected architecture/lifecycle and
integration scope. Their prior acceptance remains historical. Complete three
clean architecture reviews before implementing the replacement lifecycle;
restore each affected implementation gate before resuming M7 advancement.
M7 remains at zero and M8 remains pending. Existing distro evidence is retained,
but must not be claimed to validate the new policy implementation.

| Milestone | Deliverables and success criteria | Status | Clean streak |
| --- | --- | --- | --- |
| 1. Preserve and separate | Preserve full working-tree implementation, uncommitted changes, tests and all validation sources/evidence; verify archive hashes; record Git baseline and external-artifact dependencies; keep recoverable originals before moving old code outside the new active build. | Complete | 3 |
| 2. Architecture contract | Define owning modules/types, sealed state traits, consuming transitions, fallible finalization and recovery, confined filesystem authority, typed formatter properties, inventory migration, and requirements-to-tests traceability. Identify external qualification requirements without claiming access. | Complete: corrected registration-free architecture | 3 |
| 3. Inputs and planning | New safe Rust crate; required context-relative recipe/lock paths; exact committed context and local verified image references; one recipe mapping, immutable plan, complete stage validation/inheritance; meaningful positive/negative tests. | Complete: revised context lifecycle | 3 |
| 4. Privileged lifecycle | RAM workspace, cache-backed verified layers, kernel/idmapped mounts, environment-owned labeling exception, no command SYS_ADMIN in any capability set; private namespaces, restricted descriptors/syscalls, bounded execution/setup/teardown, cancellation and descendant quiescence; partial-setup cleanup, independent VM shutdown after supervisor death and no context/control/block-device exposure. | Reopened: registration-free lifecycle | 0 |
| 5. Direct-layer bootstrap | Trusted formatting on writable snapshot over dm-zero; tool / and output /mnt; per-step remount and policy for both; correct persisted metadata; actual carapace storage chain, clean sealing, no image conversion; failure tests. | Reopened: two-root lifecycle regression | 0 |
| 6. Full pipeline and publication | Layering, confined COPY, firmware pair extraction, DTB transforms/defaults/CARAPACE; offline default plus outbound NAT and explicit DNS; versioned deterministic CBOR inventory, self-contained OCI and atomic no-replacement output; maintained regression and integration coverage. | Reopened: replacement lifecycle integration | 0 |
| 7. Integrated qualification | Critical source/design/style review resolved; Fedora/Debian/Alpine/NixOS representative builds and boots; amd64/arm64 and TDX/SEV-SNP/CCA qualification with evidence; repository checks and CI green. Unavailable required environments remain outstanding, never passes. | In progress: qualification inventory | 0 |
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

- Historical M2 acceptance (superseded by the current reopened gate in the
  milestone table): accepted after initial findings/fixes and three clean passes:
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

### M4 domain acquisition integration (v138–v140)

- CommandPreparation::create acquires/publishes the command domain before policy
  installation and retains all supplied identities, storage and reaper on error.
  DomainCreationRollback reconciles absent/exact records under the retained lease;
  malformed or mismatched records cannot authorize cleanup. Cleanup uses the
  existing preparation deadline and never restarts setup.
- DomainTarget cleanup now has a specific owning error type. No cleanup branch
  needs to assume an impossible before-creation error means successful removal.
- v138/v139 fixture compile failures are preserved. v140 tests explicit errors
  at the publication-return boundary with real domain/record state, including
  rejection and retry of malformed/mismatched records, plus missing-domain-parent
  preparation rollback and retained ENOENT through forced unmount failure.
  The fixture now directly declares the already-cached minicbor dependency;
  its lock update was performed offline without dependency version changes.
- Namespace and filesystem mount/pin acquisition remain outside this aggregate.
  No full M4 adherence pass or milestone advancement is claimed.

- v140 completes the normal suite; v141 passes supervisor-death regression
  through aggregate domain acquisition. Host tests/Clippy and archived hashes
  pass. These results do not constitute a full M4 adherence pass.

### M4 complete single-volume preparation entry (v142 onward)

- CommandSandbox::prepare owns namespace creation/reaping, namespace pinning,
  ext4 mounting and filesystem pinning before domain/policy/helper preparation.
  It starts from Volume<Unmounted> and an installed reaper; deployment paths and
  identity allocation are explicit immutable inputs. Pre/post acquisition checks
  retain the original setup cutoff. Cleanup retains partial targets, creator
  wait ownership and assigned storage instead of dropping them through strings.
- Found and fixed cross-step reaper loss: ReapedChildren now retains the
  supervisor's exclusive reaper. ExecutedVolume::into_parts transfers it with the
  finalized volume for the next command without reinstalling subreaping.
- v143 passes two full commands in one supervisor, verifying persistent output
  and fresh temporary mounts, plus malformed-filesystem rollback. v142 compile
  failure and v143 fixture Clippy failure are preserved. v144 was an unintended
  unchanged repeat after a patch assertion; no extra coverage is claimed.
- Single-volume production acquisition/execution is now assembled. Remaining
  M4 work is a comprehensive integration/error/API/style review and fixes, then
  three clean adherence passes. No milestone advancement is claimed; M4 remains
  zero clean passes and M5–M8 remain pending.

- v145 passes the complete normal suite, including a temporarily zeroed user-
  namespace quota inside the disposable VM. The setting is restored immediately
  after preparation returns; rollback reaps the failed creator and leaves no
  pins or child processes. Host tests, root/fixture Clippy, formatting and archived
  hashes pass. No full adherence pass is claimed.

### M4 adherence review: command boundary findings (v146 onward)

- Started the source/design/style/evidence review against c808577; findings and
  remaining scope are in preservation/MILESTONE-4-REVIEW.md. Clean streak is zero.
- Found raw inherited stdout/stderr escaping the descriptor audit's 0–2 exemption.
  The contained outer receiver now supplies pipes and bounded diagnostic relays;
  commands never receive the supervisor diagnostic file descriptors. Relay errors
  reject completion. Relay threads are outside the command PID namespace, with
  no namespace transitions while they run, and diagnostic locks are per write.
- Found command completion reported before local mapped-view/namespace cleanup.
  Reporting now follows those operations. v149 injects a helper failure after
  a successful shell but before reporting; no success receipt is produced, even
  after subsequent storage cleanup retry.
- v146/v147 failed first placement attempts are preserved; v146 also exposed
  fixture logs printed only on success, now corrected. v148/v149 normal suites
  pass with the corrected placement and command FIFO checks under supervisor
  regular-file redirection.
- R2 remains open: snapshot invalid/exhausted status must be connected to
  finalization/acceptance. Review scope and three clean passes remain incomplete.

- v150 supervisor-death regression passes with the corrected relay. Host tests,
  root/fixture Clippy, formatting and evidence hashes pass. R2 and the remainder
  of the review remain open; clean adherence streak is still zero.

### M4 snapshot-health acceptance gate (v151)

- Successful flush/unmount now leads to a snapshot-status check before producing
  FinalizedVolume, followed by another check of the original teardown deadline.
  Health-check failure retains unmounted storage and the original cutoff for
  retry, without issuing an acceptance receipt.
- Host tests, strict Clippy, formatting and the existing normal disposable-VM
  suite pass. v151 evidence and hashes are preserved and verified.
- R2 remains open for a dedicated exhaustion-through-command-acceptance test.
  This is implementation progress, not a clean adherence pass. M4 remains zero;
  M5–M8 remain pending.

### M4 snapshot-exhaustion regression (v152–v153)

- Preserved v152's fixture formatting failure; corrected block size to match the
  underlying device. v153 passes the complete normal disposable-VM suite.
- Successful command followed by trusted snapshot-exhaustion injection is
  rejected during production finalization. Retry still rejects InvalidData;
  no accepted volume escapes and the mount is removed. R2 is addressed.
- Fixture Clippy/format checks and evidence hashes pass. Added the required
  exhaustion marker to runner evaluation and checked v153's existing output.
- M4 review remains incomplete with zero clean passes. M5–M8 are pending.

### M4 blocked diagnostics and lifecycle review (v154)

- Added an unread diagnostic-pipe regression. Command output begins but cannot
  finish; the finite command timeout cancels/drains the sandbox and stays failed
  through storage cleanup retry. Full normal VM suite and fixture Clippy pass;
  evidence/source hashes verified. This does not claim D-state I/O coverage.
- Reviewed production teardown/reaping/event/deadline transitions and recorded
  results and remaining API/coverage limits in MILESTONE-4-REVIEW.md. Identified
  obsolete guardian exports for preserved retirement from the active crate.
- Recorded host Cargo dependency license declarations; native/appliance audit is
  not complete. M4 remains zero clean passes; M5–M8 remain pending.

### M4 obsolete guardian API retirement

- Removed unused guardian types and upstream-watched launch/registration APIs,
  their exclusive tests and stale active comments. Exact original files/tests
  remain in preservation/retired-guardian with verified hashes and baseline.
- Root tests (46 remaining compile-fail examples), root/fixture strict Clippy and
  formatting pass. Logs preserved. No production lifecycle behavior changed and
  no new VM run is claimed for this removal.
- Continued storage/mount source review; observations recorded in the M4 review.
  Three full clean adherence passes are still outstanding; M4 streak stays zero.

### M4 command-boundary review component

- Inspected namespace/root transitions, descriptor audit and diagnostic pipes,
  capability finalization, native syscall filter, runtime mounts, admission,
  PID-1 reaping and scoped label-policy binding. Recorded a requirement/source/
  evidence matrix and source hashes under preservation/m4-review.
- No contradiction found in that component; full M4 review remains incomplete.
  No redundant runtime rerun or clean-pass claim. M4 streak remains zero.

### M4 acquisition/rollback review and R4 correction (v155)

- Traced namespace creation/pinning, mount/pin acquisition, domain publication,
  policy installation, helper launch and each rollback phase. Recorded the
  source/evidence trace in preservation/m4-review/acquisition-rollback.md.
- Fixed post-policy-directory setup expiry: retain the directory for explicit
  cleanup instead of relying on Drop and reporting no remaining resource.
  Removal failure keeps ownership and diagnostics. No policy was active there.
- Host tests/Clippy and normal VM suite pass; exact timeout boundary is reviewed
  in source, not individually injected. v155 evidence hashes verified.
- M4 full clean streak remains zero after the correction. M5–M8 remain pending.

### M4 full adherence pass 1

- Completed requirements-to-lifecycle self-review across all five mandatory
  categories on frozen implementation ed0fecd. No new finding. Source scope
  and observations are in preservation/MILESTONE-4-ADHERENCE.md and its manifest.
- Current supervisor-death regression v156 passes; protected policy/identities
  remain until independently triggered VM shutdown. Evidence hashes verified.
- Clean streak is 1 of 3. M4 is not accepted; M5–M8 remain pending.

### M4 full adherence pass 2

- Reverse-lifecycle review from accepted receipts through cleanup, admission,
  storage and namespace acquisition found no new issue. All five mandatory
  categories covered; exact 104-file scope unchanged and evidence hashes checked.
- Recorded observations in preservation/MILESTONE-4-ADHERENCE.md. This was a
  source/evidence review, not a repeated test run. Clean streak is 2 of 3.
  M4 remains unaccepted and M5–M8 remain pending.

### M4 full adherence pass 3 and acceptance

- Authority/lifetime review covered all five mandatory categories against the
  unchanged frozen scope. No new finding; three consecutive self-review passes
  are recorded in preservation/MILESTONE-4-ADHERENCE.md.
- M4 accepted for its specified single-volume privileged lifecycle. M5 may begin.
  This does not establish full builder, distro or confidential-platform completion.

### M5 started

- Read the existing formatter schema, fixed geometry and execution-receipt
  requirements. FilesystemSpec already validates ext4 capacity and label; reuse
  it without new recipe parsing or arbitrary formatter arguments.
- Recorded integrated implementation order and acceptance fixture in
  M5-INTEGRATION.md: captured trusted formatting, owned dual mounts, direct
  sealing and reopened-chain metadata checks. M5 clean streak is zero.

### M5 immutable trusted formatter capture

- Added CapturedFormatter/FormatterFile with confined file capture and immutable
  digests/bytes. Runtime file lists belong to trusted setup, never recipes.
  Captured bytes survive source mutation; duplicate/missing/nonexecutable and
  conflicting runtime paths are rejected. No source handles escape capture.
- Targeted capture tests and strict Clippy/format checks pass; logs preserved
  under preservation/m5/formatter-capture. Staging, actual formatter execution,
  dual-volume integration and sealing are still required. M5 clean streak zero.

### M5 staged formatter runtime (v157)

- Added consuming staging into a detached read-only/executable runtime view on
  the existing build tmpfs. Mutable backing remains private, source set-ID bits
  are stripped, and partial setup/removal failures retain cleanup ownership.
- Disposable-VM checks verify captured-byte independence, EROFS write-permission
  changes and shared tmpfs backing. Normal suite and root/fixture Clippy pass;
  source/log hashes verified. No formatter execution or sealing claim yet.
- M5 remains in progress, zero clean passes. M4's existing command lifecycle
  regression suite continues to pass with this additive staging path.

### M5 explicit formatter profile (v158–v160)

- Added immutable Ext4Format with validated capacity/label, fixed ext4 options,
  builder-owned configuration and deterministic CBOR profile identity. Captured
  runtime configuration rejects duplicate, ancestor and descendant collisions.
- v158/v159 passed but exposed implicit size-dependent usage selection. v160
  explicitly selects the conglobate usage profile and passes the normal VM suite
  without that warning. The fixture formats real snapshots using production
  arguments/configuration, but still invokes the VM formatter directly.
- Four formatter unit tests, strict root/fixture Clippy and formatting pass.
  All three runs are preserved with source/log/artifact hashes verified. The
  post-v160 host-only collision test is distinguished from booted sources.
- M5 remains at zero adherence passes. Captured-runtime execution, dual-volume
  commands, direct sealing and reopened-chain verification are still required.

### M5 captured formatter execution (v161–v163)

- Formatting now owns captured runtime, writable snapshot and direct trusted child
  together. Private mount/device transfer and helper chroot execute captured
  bytes; no ambient executable/library path is used after root entry. The helper
  receives only its output block device and essential private character devices.
- v161 exposed a fixture dispatch mistake, preserved before correction. v162
  passed with the original executable absent; v163 also rejects a missing loader
  and recovers both owners. Normal VM suite, 52 library tests and root/fixture
  Clippy pass. Sources, failures and artifact hashes are preserved.
- Post-v163 source review fixed cleanup cutoff origin and retry diagnostics. Host
  checks pass; timeout fault injection remains outstanding and is not inferred
  from success/nonzero tests. See M5-INTEGRATION.md for exact boundaries.
- Next connect this execution path to two-volume bootstrap and direct sealing,
  with formatter/cancellation/backing and dual-volume failure coverage. M5 remains
  at zero adherence passes; no later milestone advances.

### M5 paired volume lifetime (v164–v165)

- Added BootstrapVolumes<Mounted/Unmounted>, sealed StepStorage integration and
  pair finalization under the original teardown budget. Partial cleanup cannot
  expose either volume; retries preserve original errors and cutoff.
- v164 passes busy-output and busy-tool cases with two captured-formatter-created
  snapshots. Review added checks between unmount operations; v165 passes the
  deadline-aware pair path. 47 compile-fail doctests and root/fixture Clippy pass.
- Source/log/artifact hashes verified. This is pair ownership coverage, not yet
  dual-root command execution. Transfer, /mnt attachment, aggregate preparation,
  two-target command coverage and direct sealing remain. M5 has zero clean passes.

### M5 dual-view transfer and execution (v166–v172)

- Bootstrap packets transfer both root views with one policy-bound user namespace.
  Helper attachment retains partial ownership and unmounts output before tool.
  A reserved mnt directory is created in the unmapped tool root before idmapping.
- Preserved readiness failures, diagnostic correction, EOVERFLOW/EINVAL fixes and
  missing fixture-utility failure. v171 passes two commands; v172 passes using
  64GiB logical snapshots with 16MiB filesystems. Output UID/GID and foreign labels
  on both filesystems persist across remounts; third-filesystem labeling fails.
- 53 library tests, 47 doctests and root/fixture Clippy pass. Evidence hashes
  verified. Preparation is still assembled by fixture code; verified tool chain,
  aggregate production rollback, direct sealing and failure coverage remain.
  M5 has zero clean adherence passes; later milestones do not advance.

### M5 direct sealing and verified tool-chain fixture (v173–v175)

- Added consuming snapshot sealing in place, using devmap wire codecs and existing
  verity formatting. Retain terminating metadata; trim unused capacity; calculate
  content digest and verity tree together; expose only read/seek cursors afterward.
- v173 catches malformed fixture OCI annotations. v174 seals and verifies reopened
  output. v175 supplies a verified cache-backed tool carapace with disposable
  overlay, runs two dual-root commands, checks tool cache immutability, seals the
  separate output and reopens it through dm-verity with data/UID/GID/labels intact.
- 56 library tests, 48 doctests, root/fixture Clippy and normal VM suite pass.
  Preserved failed attempts and verified evidence hashes. Fixture preparation
  still awaits production aggregation/rollback; remaining failure tests and three
  adherence reviews are required. M5 remains active with zero clean passes.

### M5 production pair mount acquisition (v176)

- Added consuming paired mount acquisition with checks against the original
  setup cutoff, preserving both owners across partial failures. Rollback reuses
  paired unmount cleanup with the caller's fixed teardown cutoff.
- Disposable VM passed invalid-second-filesystem rollback, busy-first-mount
  recovery with original error preservation, expired setup, successful pair
  acquisition, and the complete existing normal suite. Evidence v176 preserved.
- Root/fixture Clippy and 48 doctests pass. Aggregate pair preparation and
  execution acceptance remain outstanding; M5 has zero clean adherence passes.


### M5 aggregate bootstrap preparation (v177–v178)

- Generalized the existing preparation/rollback lifecycle over sealed mounted
  storage layouts, keeping concrete single-volume aliases. Added production
  CommandSandbox::prepare_bootstrap for namespace, paired mounts, filesystem
  identity pins, containment and policy acquisition. Both volumes remain owned
  through helper launch and all partial-setup rollback states.
- Converted the dual-command bootstrap fixture to production preparation.
  v177 rejected a fixture attempt to reinstall the exclusive subreaper after
  rollback; v178 correctly uses separate failed-build processes. Domain,
  policy-install and helper-launch failures roll back both volumes cleanly.
- Full v178 normal VM suite passes, including dual commands and sealed output
  reopening; 56 library tests, 48 doctests and strict Clippy pass. Both run
  records are preserved. Production execution acceptance and remaining failure
  qualification/reviews remain; M5 clean streak is zero.


### M5 production bootstrap execution acceptance (v179–v180)

- Generalized the existing execution acceptance path over sealed storage layouts.
  Bootstrap commands now receive a success receipt only after their own successful
  exit and finalization of both filesystems/snapshots. Cleanup failures retain the
  command outcome; rejected storage remains inaccessible until cleanup completes.
- Converted the two-command bootstrap fixture to production execute. Verified
  exit 17, timeout, nonzero-plus-busy and success-plus-busy outcomes and retries.
  v179 caught a nonexistent timeout-fixture executable; v180 uses shell builtins
  and passes the complete normal VM suite, sealing and reopening included.
- 56 library tests, 49 doctests and strict Clippy pass. Exact failed/successful
  evidence and post-run strengthened doctest source preserved. Remaining M5
  failure/metadata qualification and all three adherence passes stay outstanding.


### M5 metadata and formatter failure qualification (v181–v182)

- v181 full normal VM suite verifies ACL UID789, image owner123:456, modes0640/
  0755, file capabilities without a build-root identity, and foreign SELinux
  labels after command remount and sealed-chain reopening.
- v182 targeted VM verifies the fixed 30-second formatter timeout, explicit
  cancellation/reaping and resource recovery; real mke2fs on undersized COW
  storage is rejected with invalid snapshot status and releases all backing.
- Strict fixture Clippy passes; exact evidence retained. No production source
  changes. Initial listed formatter/metadata checks are now covered; full M5
  adherence reviews and any resulting fixes remain. Clean streak stays zero.


### M5 adherence review: formatter input lifetime (R1, v183–v186)

- Initial source review found that dropping unfinished Formatting or its wait
  failure could release the runtime and request snapshot teardown after only
  best-effort process reaping. Retain both inputs until an explicit successful
  wait/cancel; abandoned owners retain them through VM disposal.
- v183 targeted formatter regressions, v184/v185 terminal drop-retention tests,
  and v186 full normal VM suite pass. 56 library tests, 49 doctests and strict
  root/fixture Clippy pass; exact sources/evidence retained.
- R1 addressed. Initial review coverage and remaining work are recorded in
  preservation/MILESTONE-5-ADHERENCE.md. Full review remains incomplete and the
  M5 clean streak stays at zero; no later milestone is advanced.


### M5 full adherence pass 1

- Requirements/ownership-first review of frozen implementation f8ad64d and affected
  interfaces found no new issue after R1. Covered requirements, Rust style,
  lifetime/error/deadline paths, meaningful evidence, preservation and scope.
- Verified selected VM verdicts and all recorded artifact hashes; matched current
  changed source against archived v186 source. Freeze recorded in
  preservation/m5-review/scope.json; detailed pass in MILESTONE-5-ADHERENCE.md.
- Clean self-review streak 1/3. M5 remains unaccepted; M6–M8 stay pending.


### M5 second review finding R2: architecture/source reconciliation

- Reverse-lifecycle review found stale ARCHITECTURE.md statements assigning
  readonly assembly to a pinned carapace API. The accepted M4 ReadOnlyChain
  refinement already records why devmap/lodown ownership is required for explicit
  partial-setup and teardown. Reconciled that existing decision in the architecture
  document; no source/dependency/format/trust-model change.
- Architecture is part of the frozen scope, so clean streak resets to 0/3.
  Prior pass/scope remain historical. R2 and inspected coverage are recorded in
  preservation/MILESTONE-5-ADHERENCE.md. No redundant runtime tests; M6 stays gated.


### M5 restarted adherence pass 1 after R2

- Verified the new scope-v2 manifest and reviewed requirements, formatter and
  paired lifecycle, public acceptance authority, Rust style and evidence.
- Rehashed all stored evidence/artifacts for v176, v178, v180, v181 and v183–v186;
  all match. No runtime rerun or independent qualification claim.
- Reconciled the integration ledger's sealing API wording with R2 as well.
  Production source is unchanged. Detailed review is in the M5 adherence record.
- Restarted clean streak 1/3. M5 is not accepted; M6–M8 remain pending.


### M5 restarted adherence pass 2

- Reverse-lifecycle full self-review on unchanged scope-v2 is clean. Checked
  requirements, consuming/typestate APIs, backing/identity ownership, explicit
  flush/unmount versus destructor fallback, retries, evidence and preservation.
- Reviewed concrete failure assertions and existing test/Clippy logs; no runtime
  rerun was counted as review and no production code changed.
- Clean streak 2/3; milestone summary synchronized. M6–M8 remain pending.


### M5 restarted adherence pass 3 and acceptance

- Public-API/acceptance-first full self-review is clean on unchanged scope-v2.
  All 120 frozen hashes and 41 v186 archived source/build/fixture files match.
- Requirements, Rust style, resource/state authority, failure paths, evidence
  and preservation reviewed; limits recorded in the M5 adherence ledger.
- M5 accepted with 3/3 consecutive clean self-review passes. M6 starts next;
  no full builder, distro/boot or confidential-platform claim is inferred.

### M6 integration audit and start

- Inspected BuildRequest, CapturedBuild/ResolvedBuild, BuildPlan, local cache
  resolution, COPY descriptions and transform/template descriptions. These feed
  actual M4/M5 execution APIs but do not yet form a runnable builder.
- M6-INTEGRATION.md records the complete integration order: real pass-through
  CLI/output/inventory first, then bootstrap/layering/COPY, firmware/transforms,
  enabled networking and the full regression matrix. No subset is acceptance.
- Keep output ancestry separate from tool inputs and immutable source identities
  separate from active resource ownership; reuse the existing lifecycle.
- M6 has zero clean passes. The previous goal turn was progress (M5 pass 2);
  this turn accepts M5 and establishes the concrete M6 source/API boundary.

### M6 output storage transaction

- Added private sibling staging with parent-directory authority, exact-stream
  verified cache copying, content-addressed blob writes, fixed metadata names,
  and atomic no-replacement publication retaining staging on failure.
- The transaction does not assert build/manifest/inventory validity; the next
  integration is the output assembler and version-two inventory, then real CLI.
- Three output tests, 12 cache tests and strict all-target Clippy pass. Preserved
  source/log hashes under preservation/m6/output-storage. No privileged host
  operations. M6 remains active with zero adherence passes.

### M6 inherited artifact assembly

- Added immutable AssembledImage from fresh staging, preserving exact base
  manifest/configuration/all layer bytes, materializing canonical empty config,
  and verifying staged content and chain before returning the owner.
- Output ancestry selection still belongs to the upcoming coordinator; inventory
  and publication are not exposed by this assembled owner yet.
- Five output tests and strict all-target Clippy pass. Exact-manifest/extensions,
  source-cache removal, empty/real config and missing-reference cleanup tested.
  Preserved initial compile failure and passing evidence; no host privileged work.
- Next connect version-two inventory and the actual pass-through CLI path. M6
  remains active, zero adherence passes; later stage/qualification work remains.

### M6 version-two inventory wire format

- Added immutable Inventory and private typed version-two records, deterministic
  CBOR encoder and strict decoder. Schema covers formatter receipts, one selected
  recipe and lock, ordered network steps, typed digest bytes and transform status.
- Rejects unknown/duplicate/noncanonical/trailing data and invalid identities,
  ordering, formatter metadata and output combinations. No public mutable wire
  record or execution-success claim is exposed by decoding.
- Five focused inventory tests, all 66 library tests, strict all-target Clippy
  and command-fixture compilation pass. Added safe ciborium/serde_bytes libraries
  and updated both root and active fixture locks offline. Evidence preserved.
- Next bind actual input/output identities and completed-stage receipts to this
  schema, validate against the staged manifest, and connect CLI publication.
  M6 remains active with zero adherence passes.

### M6 pass-through CLI and inventory binding

- Connected standalone helper dispatch and build CLI to committed context/local
  cache, private read-only tmpfs checkout, inherited assembly, actual input/output
  inventory and atomic publication after context cleanup.
- Private PassThrough and CompletedImage types gate this path. Unsupported
  bootstrap/directive/firmware/transform work fails explicitly; these remain
  required implementation work. Inventory identities are cross-checked against
  assembled output, with exact authored/default transform identity documented.
- v187 disposable VM passes the actual CLI, independent inventory-field and blob
  checks, source-cache removal, committed recipe versus modified worktree,
  destination collision and failure-without-publication cases. v188 adds exact
  output byte export to preserved evidence. No host mounts/policy operations.
- 66 library tests, 49 doctests, root and fixture strict Clippy pass. Initial
  non-exhaustive Git hash-kind compile error was corrected and retained in logs.
  M6 remains zero adherence passes; remaining execution pipeline work is open.

- v188 also passes, with exact index/manifest/CBOR artifacts preserved. Host
  cross-check confirms exported index binds the exact manifest. Evidence v187/
  v188 includes sources, contexts, cache fixtures, logs and boot artifact hashes;
  initial pre-export fixture reconstruction is labeled. Remaining M6 work includes
  stage execution, COPY, firmware/transforms, enabled networking and ORAS coverage.

### M6 DTB transformation execution

- Previous turn was a status-only response (no implementation progress). Resumed
  with safe shared-devtree execution of validated transform operations. All set
  types, remove, conditional existence and regex operate on private owned state;
  callers retain input bytes on failure. Authored empty still requires a DTB.
- Default sequence now has one canonical recipe-owned definition, used by both
  parsing/execution and inventory identity. Execution records observed skip
  disposition rather than assuming it from selection. No-op output preserves
  exact input bytes; edits preserve reservations, compatibility and boot CPU.
- Regex checks a single NUL-terminated UTF-8 string before matching, replaces
  every match, and expands no variables on zero matches. Expanded NUL is rejected.
- 70 library tests, 49 doctests, strict all-target Clippy and integration-fixture
  compilation pass. Both dependency locks include the preserved devtree pin.
  Source snapshots and logs are in preservation/m6/transform-execution.
- This executor is not yet connected to CLI publication: manifest replacement,
  receipt-based inventory binding and end-to-end transform cases are next.
  No new VM run or full adherence pass is claimed. M6 remains at zero passes.

### M6 DTB transforms connected to publication

- Previous turn progressed in cff57e1. The inherited-image coordinator now reads
  the verified DTB, executes the selected transformation with the verified root,
  and binds actual target/skip disposition into inventory before publication.
  The selected path is renamed InheritedBuild: it is no longer pass-through-only.
- Assembly replaces DTB content/digest/size without retaining obsolete blobs.
  Unrelated manifest fields survive; stale inline data/URLs are removed on edits.
  No-op edits retain exact manifest bytes. Final manifest/references/chain are
  revalidated from staged output before inventory/publication.
- 71 library tests, 49 doctests and root/fixture strict Clippy pass. The new
  assembly regression checks changed identity, extension preservation and
  independence from the source cache after deleting it.
- Disposable VM v189 passed default/authored edits but failed the test's expected
  error text for missing DTB: input validation rejects earlier with its own
  correct diagnostic. Preserved this failure, corrected the assertion, and added
  the transform marker to the runner's acceptance conditions. v190 passes actual
  CLI default/authored edits, final DTB/inventory identities, unchanged boot CPU,
  all output blob digests and no-publication for missing DTB/property. Prior
  collision/unsupported/missing-cache/pass-through cases still pass.
- Evidence and source snapshots for both runs are preserved under validation/
  rewrite-command-integration/evidence/v189 and v190. No image-authored command,
  distro boot, ORAS or confidential-platform qualification is inferred. M6 remains
  at zero adherence passes; stage execution, COPY, firmware and networking remain.

### M6 committed COPY source selection

- Previous turn progressed in fde0f6b. Added immutable CopySources borrowing the
  captured Git tree: ordered source selection, directory contents, explicit
  symlink resolution, nested-link preservation and confined cycle/escape checks.
  It never opens caller worktree paths or exposes the context to commands.
- Reviewed Docker COPY documentation and fsutil copy.go. Explicit prefixes may
  follow directory links; wildcard traversal treats links as leaves. Escaped
  wildcard characters do not begin traversal, whereas escaping in a wildcard
  suffix participates in matching. Added regressions for those distinctions.
- 75 library tests and strict all-target Clippy pass. Source/reference bytes and
  successive logs retained in preservation/m6/copy-selection. These are source-
  grounded regressions, not an executed Docker/BuildKit compatibility claim.
- Destination writing, metadata application and pipeline execution are still
  required; source selection alone is not a working COPY directive. M6 remains
  zero adherence passes. No privileged test was needed for immutable-tree work.

### M6 COPY permission execution and reference comparison

- Previous turn progressed with immutable source selection. While preparing
  destination metadata, source comparison found the old evaluator incorrectly
  used evolving modes for X and combined sequential class-copy operations.
  Added CopyMode::apply over validated descriptions, with original-mode X,
  ordered class copies, zero umask and scoped special-bit handling.
- Executed tonistiigi/dchapes-mode at 73d941a28323f67dc09aefc69afd2770dc9dfc8b
  using checksum-verified Go 1.27.3. Compared 429 expressions, files/directories,
  and all 4096 starting permission patterns: 3,514,368 outputs agree. Preserved
  deterministic digest vectors in tests/data/copy-mode-reference.json for normal
  Rust regression tests without a runtime Go dependency.
- Preserved reference source/license, generator, commit, toolchain checksum,
  logs and source snapshot in preservation/m6/copy-mode. First Go run failed
  because /tmp quota was exhausted; established /var/tmp TMPDIR fixed it.
- All 77 library tests and strict all-target Clippy pass. This turn completed
  permission calculation, not the originally planned destination writer.
  Confined destination access, account lookup, metadata syscalls and pipeline
  integration remain required. No privileged filesystem behavior or complete
  COPY compatibility is claimed. M6 remains zero adherence passes.

### M6 confined COPY destination writer

- Previous turn progressed on reference-validated permissions. Added descriptor-
  relative destination writing and Volume<Mounted>::copy with an exclusive borrow.
  A compile-fail test rejects copying while a command root view is outstanding.
  All image path resolution begins at the pinned volume root; absolute symlinks
  have image-root semantics. O_PATH inspection does not open devices/FIFOs.
- Regular destinations are unlinked and exclusively recreated, avoiding writes
  through existing hardlinks. Nested destination links are replaced for file
  copies. Directory conflicts fail; selected directory contents merge, nested
  source symlinks remain links. Owner/group lookup uses bounded regular image
  account files; numeric sentinel IDs fail; ownership precedes final mode.
- Host regressions cover committed versus dirty bytes, symlinks, hardlinks,
  conflicts, modes, account reads and FIFO rejection. 80 library tests, 50
  doctests, and root/fixture strict Clippy pass.
- v191 full normal disposable-VM suite passes. New mounted-volume check copies
  committed input to ext4 output and verifies UID 123/GID 456/mode 0640 before
  unmount/seal. Existing lifecycle/sealing checks remain green. Added the marker
  to future runner acceptance after inspecting this run's success. Raw evidence,
  context bundle, cache fixtures, compressed source snapshot and artifact hashes
  preserved under validation/rewrite-command-integration/evidence/v191.
- This is a mounted-volume API, not yet complete CLI directive orchestration.
  Bootstrap /mnt routing, deadline integration, comprehensive COPY compatibility
  and emitted-image metadata qualification remain required. M6 zero passes.

### M6 bootstrap COPY across tool and output roots

- Previous turn progressed with Volume<Mounted>::copy. Added an immutable
  two-root layout to the confined writer and BootstrapVolumes<Mounted>::copy,
  exclusively borrowing both volumes. /mnt selects the output handle; image
  symlinks may cross in either direction. Absolute targets restart at the tool
  root, and /mnt/.. returns there. Tool backing contents beneath /mnt stay hidden.
  The real reserved tool mountpoint is validated with the existing preparation API.
- Host regressions verify forward aliases, absolute/relative return links,
  mountpoint component boundaries and hidden backing contents. 81 library tests,
  50 doctests, root/fixture strict Clippy pass.
- Added actual command/COPY/command validation: command one creates cross-root
  symlinks; trusted COPY remounts and writes through both; command two remounts,
  verifies both contents plus UID/GID/mode and absent context. v192 stopped
  before COPY because the minimal fixture lacked ln; retained its failed evidence,
  added ln/stat to the tool fixture, and v193 passes the full normal VM suite.
- Evidence, fixture sources/context, source archives and hashes retained in
  validation/rewrite-command-integration/evidence/v192 and v193. These exercise
  production storage/COPY/command APIs in sequence, not complete CLI recipe
  orchestration. Trusted-work deadline integration and full stage coordinator
  remain necessary. M6 stays at zero adherence passes.

### M6 production directive runner

- Previous turn progressed with bootstrap COPY routing. Added
  CommandSandbox::directives for sealed single-volume and bootstrap layouts.
  It executes validated directives in order, remounts each RUN/COPY, uses recipe
  RUN timeouts, and supplies an explicit output-root CARAPACE as plain hex.
  Enabled networking fails preflight until owned network integration exists.
- CompletedDirectives has no public constructor. Preparation/execution failures
  retain their existing lifecycle owners; COPY failure retains original error,
  storage/reaper, unfinished cleanup and its original fixed teardown cutoff.
  Cleanup retry can return only rejected storage, never an accepted stage.
  COPY checks fixed setup/teardown budgets and snapshot health before acceptance;
  synchronous kernel operations may return late, but late completion is rejected.
- v194 full normal VM suite passes after replacing the hand-written positive
  fixture sequence with the production runner over a parsed mixed RUN/COPY recipe.
  Both commands verify CARAPACE; the second verifies remounted copied data and
  metadata. Command failures retain prior dedicated lifecycle coverage; broader
  directive-runner failure/CLI publication coverage remains necessary.
- 81 library tests, 50 doctests and root strict Clippy pass. Fixture Clippy found
  an unnecessary explicit drop of ChildReaper (no destructor). Removed it;
  fixture strict Clippy then passes. Preserved original VM source, initial lint
  error, corrected fixture and passing log under evidence/v194 with hashes.
- This connects execution APIs into production recipe sequencing, not yet CLI
  image-stage construction/sealing/inventory/publication. Those connections and
  enabled networking remain required. M6 zero adherence passes; goal active.

### M6 produced-scute output assembly

- Previous turn progressed with production directive sequencing. Added
  AssembledImage::extend to append a sealed scute while preserving inherited
  config/components/unknown manifest fields and updating the declared root.
  Requires the scute parent to match the selected base; final staging is reopened
  and all referenced bytes and the full ancestry recomputed before acceptance.
- Produced scutes stream through a 64KiB buffer into a private pending file.
  Size/digest/EOF must match before installation under a content digest name.
  New host regression covers truncation, trailing data, wrong digest, exact bytes
  and cleanup of rejected pending files. No whole-scute memory buffer is added.
- v195 full normal VM suite passes. It creates a real child snapshot over the
  sealed bootstrap layer, seals it, assembles two layers, checks ordered roots,
  inherited descriptor preservation and absence of premature publication. An
  intentionally wrong parent is rejected and its staging explicitly discarded.
- 82 library tests, 50 doctests, root and fixture strict Clippy pass. Evidence,
  source archive, fixture context/cache and artifact hashes preserved in
  validation/rewrite-command-integration/evidence/v195.
- This artifact constructor proves content/ancestry, not command success. The
  coordinator must bind CompletedDirectives, finalize DTB/firmware and inventory,
  and publish only after cleanup. CLI stage integration remains open; M6 zero
  adherence passes and goal active.

### M6 fresh bootstrap output assembly

- The preceding status-only user turn made no implementation progress. Resumed
  the pending production bootstrap assembler and completed its validation.
- AssembledImage::bootstrap requires a zero-parent sealed scute and builds a fresh
  canonical-config, single-layer manifest, with no inherited tool components.
  Common final layout construction reopens and verifies all blobs and the chain.
  Assembly errors retain staging for explicit cleanup; construction does not
  authorize publication or prove successful recipe execution.
- v196 full disposable-VM suite passes, including the actual sealed bootstrap
  output, exactly one output layer, canonical config, matching root and rejection
  of a nonzero-parent child. Both success and rejection staging are discarded;
  the destination remains unpublished. Existing fixture cache construction is
  still used for independent filesystem inspection.
- Full cargo test passes (82 library tests, integration suites, 50 doctests);
  root and fixture strict Clippy pass. Logs, source and context/cache evidence
  preserved in validation/rewrite-command-integration/evidence/v196.
- M6 remains zero adherence passes. Next critical integration is CLI ownership
  of runtime mount/policy/cgroup paths and command-stage execution, followed by
  completed-stage inventory binding and publication. No full CLI build claimed.

### M6 build-owned runtime control mounts

- Previous goal turn progressed in 9c39c04. Added storage::Runtime to own a
  protected attachment of an exclusive subdirectory of the existing workspace
  tmpfs, a private bpffs mount, a cgroup2 view with an exclusive build subtree,
  and the recovery lease. RuntimePaths borrows the owner and provides canonical
  paths to helpers; cleanup retains the original parent descriptor. No second
  workspace tmpfs or command-visible control paths are introduced.
- Acquisition uses exclusive temporary directories for both backing and target;
  a collision cannot authorize removal of someone else's entry. Setup uses a
  fixed budget; consuming close and failed cleanup preserve a fixed deadline.
  Recovery/volume entries, policy pins and nonempty cgroup subtrees block cleanup.
  Only empty-directory removal and non-lazy unmounts are used. Unfinished owners
  retain mounts until VM disposal. A compile-fail test prevents closing Runtime
  while its sandbox paths are borrowed.
- v197 caught default bpffs permissions rejected by existing policy validation;
  archived failure before correcting the new mount to mode 0700. v198 passed the
  full normal suite. Source review then improved exclusive backing-directory
  acquisition; v199 validates the final implementation and also passes.
- The actual two-command sandbox sequence and command preparation failure paths
  now use Runtime instead of fixture-owned control mounts. Tests also reject an
  unprotected parent and prove recovery/volume/cgroup entries prevent close,
  remain present after rejection, and permit explicit cleanup after removal.
- Full host tests pass (82 library tests, integration suites, 51 doctests); root
  and fixture strict Clippy pass. All three VM runs, sources, contexts, cache
  fixtures and provenance are preserved under evidence/v197 through v199.
- M6 remains zero adherence passes. Runtime acquisition fault injection and the
  CLI coordinator are still outstanding. Next connect this owner to carapace
  directives, finalized storage, produced-origin inventory and publication;
  firmware and enabled networking remain required. No complete CLI build claim.

### M6 executable carapace CLI path

- Previous goal turn progressed in b7ac903. Connected Pipeline to a validated
  CarapaceBuild for recipes with carapace directives. It owns the runtime, executes
  each directive against a fresh snapshot, seals it only after successful storage
  finalization, and publishes after runtime and private-context cleanup.
- Preserved DESIGN.md's one-scute-per-directive contract. ReadOnlyChain::extend
  consumes the preceding chain and activates a shared immutable SealedScute
  directly from RAM through dm-verity. PreparedScute now distinguishes cached
  inputs from produced inputs; inherited aligned layers remain cache-backed.
  Appending a layer checks its parent. The existing stack owns ordered teardown.
- Output assembly appends all produced descriptors, checks their ordered ancestry,
  applies final DTB edits using the resulting root, and verifies the staged image.
  Inventory binds produced digests in output-chain order and marks only those
  entries produced. Sparse COW capacity accommodates the full logical device plus
  exception metadata; physical allocation still obeys ordinary tmpfs limits.
- Rejected directives preserve their primary error while attempting bounded
  cleanup; rejected storage is discarded and cannot become an accepted receipt.
  Unfinished kernel resources retain the existing dedicated-VM disposal fallback.
  Bootstrap/firmware coordination and enabled networking remain explicit errors.
- v200 full normal VM suite passes, including an actual conglobate CLI invocation
  on a committed RUN/COPY/RUN recipe. It produces exactly three new scutes above
  one inherited scute; inventory origins match; committed COPY bytes win over
  dirty worktree bytes; the published chain remounts with the expected files.
  A separate command exits 37 and leaves neither output nor runtime attachment.
- v201 pass-through/DTB CLI regressions pass. The test harness now includes the
  actual CLI in normal runs and has a 150-second observation limit; production
  command timeouts are unchanged. Host cargo test passes (82 library tests,
  integration suites, 51 doctests), as do root/fixture strict Clippy. Evidence,
  source, executable hashes and v201 exported inventory/index are preserved.
- M6 is still zero adherence passes. Remaining work includes bootstrap and
  firmware CLI coordination, enabled NAT/DNS, complete output/failure matrix,
  runtime acquisition fault injection, ORAS interoperability and the three-pass
  gate. Full distro and platform qualification remains M7.

### M6 firmware/DTB measurement contract correction

- Previous goal turn progressed in e56536b. Reviewing firmware extraction against
  DESIGN.md section 10 found an omitted existing-publication requirement: changed
  DTBs were accepted with opaque PMI bytes. Earlier transform fixtures proved
  editing/publication plumbing, not the required firmware measurement boundary.
  M6 remains zero adherence passes; this finding is corrected before extending
  firmware publication.
- Added immutable DtbContract using safe goblin PE parsing and pinned independent
  pmi target-schema types. Every .pmi target must be recognized (VM/SEV/TDX/CCA),
  decode strictly, have exactly one external dt:dtb fill, and name an unambiguous
  Zero section with enough reserved capacity. Duplicate CBOR keys/target names,
  unknown contracts, malformed data and signed SEV identities reject the edit.
  The strictest target capacity applies. VM register schema follows PE machine.
- Output assembly gates changed DTB bytes before acceptance for both inherited
  and derived images. The PMI is captured with its verified size/digest and is
  not constrained by the small-metadata limit. Unchanged DTBs skip contract
  parsing, preserving opaque/custom firmware behavior. No resealing is claimed.
- Replaced misleading opaque-success fixtures with independently encoded minimal
  structured PMI. Added real CLI rejection for opaque-plus-changed-DTB and success
  with explicit unchanged transforms. v202 pipeline suite passes. Full host tests
  pass (85 library tests, integration suites, 51 doctests); final additional
  mixed-capacity and duplicate-CBOR assertions pass; root/fixture Clippy pass.
  Evidence/source/dependency locks/exports/hashes are preserved under v202.
- Firmware command execution/extraction, bootstrap coordinator, enabled NAT/DNS,
  remaining matrix/fault coverage, ORAS and M6 reviews remain outstanding. This
  is a correction to the actual publication boundary, not completion of firmware
  execution or launch/platform qualification.

### M6 firmware execution, paired extraction and publication

- Previous goal turn progressed in 1edd17b. Added an immutable ExtractedFirmware
  owner and consuming finalization into FinalizedFirmware. An exclusive mounted
  volume borrow captures both declared regular files through IN_ROOT openat2,
  with no magic links or submount crossing. O_PATH metadata checks precede
  reopening the pinned inode, so FIFOs/devices are never activated for reading.
  Absolute and parent symlinks retain image-root semantics.
- Captured files stream into the existing RAM workspace with measured size/digest
  and fixed extraction budget. No mutable/raw file authority escapes. Finalization
  applies the requested transform and checks the captured PMI contract whenever
  DTB bytes change. The coordinator separately requires successful commands,
  quiescence, unmount/storage teardown and runtime/context cleanup.
- FirmwareBuild now supports firmware-only builds and replacement on an unchanged
  inherited carapace. All firmware directives share a disposable writable tool
  root, and both outputs are extracted after final command completion. Tool
  filesystem changes never produce output scutes. CARAPACE comes only from the
  output base, and is absent for firmware-only builds.
- Output assembly emits a fresh canonical firmware-only image or preserves all
  non-firmware components/config/annotations of the selected output base while
  replacing PMI and DTB together. Inventory/publication use the existing checked
  successful-output path. Shared sparse snapshot capacity moved onto Workspace.
- v203 full normal VM suite passes, including real CLI firmware-only and
  inherited-base builds, absolute-symlink extraction, expected PMI/DTB bytes,
  zero/one inherited scute, unchanged base root and inventory shape. Missing DTB
  leaves no output or runtime attachment. Host tests cover confined paths,
  missing/cyclic links and rejection of directories/FIFOs before opening them.
- Full host tests pass (86 library tests, integration suites, 51 doctests), as do
  root/fixture strict Clippy. Logs, fixture sources/contexts/cache and executable
  hashes preserved in evidence/v203. No complete M6 adherence pass claimed.
- Still required: composition of newly derived carapace layers with firmware,
  bootstrap CLI coordination, enabled NAT/DNS, remaining matrix/failure coverage,
  ORAS interoperability and three clean reviews. M6 remains active at zero.

### M6 retained carapace followed by firmware

- Previous goal turn progressed in 88e3ddd. Split successful carapace execution
  from final publication: DerivedCarapace carries sealed scutes, their live
  read-only chain and the exclusive child reaper to the next stage. Standalone
  carapace publication consumes the same result; no duplicate executor is added.
- Firmware with implicit tools now executes on that newly derived chain. An
  explicit tool reference closes the output chain and activates the requested
  cached tool chain, retaining immutable produced bytes for publication. Both
  paths receive CARAPACE from the final output root, never the alternate tool.
  The same reaper transfers across stages instead of reinstalling supervision.
- Paired firmware assembly appends the successful produced scutes to the selected
  output base with ordered parent checks, then publishes the replacement pair.
  Inventory binds produced origins. DTB transformation occurs after firmware
  production; inherited firmware is not prematurely transformed before replacement.
- v204 full normal VM suite passes. Actual combined CLI builds cover implicit
  tools (new file visible), explicit original tools (new file absent), PMI bytes
  equal to the resulting CARAPACE root, different input/output roots, exactly one
  new scute, correct inventory origins, and successful output-chain remounts.
  Retained carapace content survives; disposable firmware files do not. A missing
  firmware output after successful derivation prevents publication and leaves no
  runtime attachment.
- Full host tests pass (86 library tests, integration suites, 51 doctests), with
  root/fixture strict Clippy. Evidence/source/executable hashes preserved in v204.
  M6 remains zero adherence passes. Bootstrap coordination, enabled networking,
  remaining output/failure/variable-expansion matrix, runtime acquisition faults,
  ORAS interoperability and the three-review gate remain open.

### M6 formatter execution receipts

- Previous status-only turn was no implementation progress; resumed work against
  current source. Bootstrap CLI integration revealed that successful formatter
  execution had no inventory receipt transition. This gap is now closed without
  treating a planned formatter invocation as execution evidence.
- FormattedSnapshot::into_recorded_parts consumes a successful/reaped formatter
  result and returns its resource owners plus immutable FormatReceipt. The receipt
  retains exact executable/runtime identities and effective typed ext4 format.
  Its constructor is private; recipe plans and decoded inventory cannot mint it.
- Inventory::with_formatter consumes that receipt into the existing version-two
  schema, including profile/configuration digests, runtime paths/digests, arguments
  and filesystem geometry. Duplicate bootstrap formatter attachment is rejected.
  Resource cleanup, command success, sealing and publication remain separate gates.
- Host tests (86 library tests, integration suites, 52 doctests), root/fixture
  strict Clippy and formatting pass. v205 full normal disposable-VM suite passed
  in 132.84 guest seconds. The fixture compares receipt identities to the runtime
  of an actual successful format, then tests CBOR encoding/decoding in memory.
  That probe document is not published as a bootstrap CLI build. Source, executable
  identities and evidence are preserved under validation/rewrite-command-integration/evidence/v205.
- Bootstrap CLI orchestration remains unfinished. In particular the production
  CLI still needs trusted formatter runtime selection (currently supplied only
  by the validation fixture), composition with later stages, and receipt attachment
  after successful cleanup. Networking, remaining matrix/fault tests, ORAS and
  three full M6 adherence passes remain outstanding. M6 streak remains zero.

### M6 standalone bootstrap CLI

- Previous goal turn progressed in 50ec86b (formatter execution receipts).
  Connected standalone bootstrap to the actual CLI. The trusted environment now
  supplies --formatter-runtime PATH: strict YAML root/executable/files selection,
  separate from authored recipe properties. Relative roots resolve against that
  manifest; captured executable/runtime bytes and builder-owned configuration are
  retained/measured. Missing runtime files fail; no ambient fallback or downloads.
  DESIGN.md documents this interface. Unit coverage rejects duplicate/unknown
  fields, duplicate runtime paths, escaping paths and missing files.
- BootstrapBuild creates a zero-backed writable snapshot, executes the captured
  typed formatter, closes its runtime, activates the verified cache-backed tools
  with a disposable writable overlay, and executes bootstrap RUN/COPY through the
  existing two-volume directive runner. Tool changes are discarded; output is
  sealed directly as a zero-parent scute. No intermediate filesystem image/import.
- Command failures use retained pair cleanup, formatter failures reclaim reaped
  resources (or retain unfinished ownership), and runtime cleanup is required
  before returning publishable state. Default DTB handling and the actual
  formatter receipt enter the inventory; private context cleanup still precedes
  atomic output publication. Tool-carapace components never become ancestry.
- v206 full normal disposable-VM suite passes (148.56 guest seconds). New CLI
  cases cover fresh output, committed-context COPY despite dirty worktree,
  command failure, missing --formatter-runtime, and an incomplete loader runtime.
  The output is reopened through the verified storage chain: one produced scute,
  requested files present, tool /bin and disposable tool writes absent, no inherited
  firmware, formatter label/capacity recorded. Failures publish no directory and
  leave no runtime attachment. Source/evidence/executable hashes archived under
  validation/rewrite-command-integration/evidence/v206. Host tests (87 library,
  integration suites, 52 doctests), strict root/fixture Clippy and formatting pass.
  Host observation timeout increased to 180 seconds for the additional CLI cases;
  production setup/command/teardown deadlines are unchanged.
- This enables standalone bootstrap only (including an empty carapace stage).
  Bootstrap followed by retained derivation or firmware remains explicitly
  rejected and required next. Enabled NAT/DNS, remaining matrix/variable/fault
  coverage, ORAS and three full adherence passes remain outstanding. M6 stays
  active at zero clean passes; no full distro/platform qualification is claimed.

### M6 bootstrap followed by retained carapace layers

- Previous goal turn progressed in 83486fe. Bootstrap output can now feed retained
  carapace directives directly. ReadOnlyChain::from_root accepts only a sealed
  zero-parent root and activates its RAM-backed scute without an intermediate
  cache image/import. Existing cached-base execution and this fresh-root path share
  DerivedCarapace::derive, consuming active-chain/scute/reaper ownership and sealing
  one layer per directive. Bootstrap's original tool components remain excluded.
- Fresh output assembly accepts the complete ordered produced chain, checks every
  parent starting at zero, streams every scute, sets the final root annotation,
  and verifies the self-contained staged layout. Inventory binds all layers as
  produced and retains the single actual formatter execution receipt. No output
  publication occurs until all commands and cleanup succeed.
- v207/v208 exposed a synthetic-root limitation: the fixture installed a shell
  and libraries but omitted /dev, /proc and /sys mountpoint directories required
  by the existing command runtime. Failure surfaced as registration peer loss,
  truncated helper diagnostics and retained runtime cleanup. Preserved both
  failures before changes. Supervisor preflight now validates the same confined
  runtime targets before policy/command launch, giving an explicit missing-target
  error and normal rollback. Helper diagnostics render before writing; this does
  not promise delivery after cancellation and adds no synchronization sleep.
- The valid fixture explicitly supplies its root skeleton through measured COPY.
  A separate missing-target recipe verifies failure names /dev, publishes nothing,
  and leaves no runtime attachment. Automatic creation/restoration of absent
  runtime mountpoints is not implemented: current command roots must supply /dev,
  /proc and /sys (or valid confined symlink targets). This limitation remains visible
  for subsequent distro qualification; no arbitrary-root compatibility is claimed.
- v209 reached all expected CLI outcomes but failed an existing firmware inventory
  assertion because a broad fixture edit accidentally changed its conditional.
  Preserved the run and restored the original firmware assertion without changing
  production behavior. v210 full normal VM suite passes (176.62 guest seconds),
  including bootstrap -> COPY/RUN/COPY, four produced scutes, committed copy bytes,
  retained data, no disposable tool changes, real output-chain reopening, failed
  later RUN with no output, and rejection of a nonzero-parent root constructor.
- Evidence/source/executable identities for v207-v210 are preserved under
  validation/rewrite-command-integration/evidence. Host tests (87 library tests,
  integration suites, 52 doctests), strict root/fixture Clippy and formatting pass.
  Host suite observation timeout is now 210 seconds for the expanded cases;
  production execution/setup/teardown deadlines are unchanged.
- Firmware after bootstrap remains explicitly unsupported and is the next
  composition path. Enabled networking, remaining variable/matrix/fault coverage,
  ORAS, three full M6 adherence passes and all later milestones remain required.
  M6 stays active at zero clean passes; these checks are not a milestone review.

### M6 firmware after bootstrap

- Previous goal turn progressed in 2f8c9aa. Removed the bootstrap/firmware
  composition rejection. FirmwareTools now owns the selected read-only tool chain,
  separate output-root identity and transferred child reaper. Both cached-base
  FirmwareBuild and BootstrapBuild consume the same firmware command/extraction
  implementation; no second executor or intermediate cache publication is added.
- Bootstrap with retained directives transfers its active derived chain to
  implicit firmware tools. Explicit firmware.from closes that chain and activates
  the selected cached tools while retaining immutable output scutes. Without
  retained directives, implicit firmware activates the fresh sealed root directly;
  explicit tools do not require a runnable shell in the retained bootstrap output.
  CARAPACE always denotes the output root, not the selected tool root.
- Fresh firmware output assembly now accepts the complete zero-parent produced
  chain, checks each parent, supplies canonical carapace metadata, and appends the
  captured PMI/DTB pair. No bootstrap tool components or disposable firmware writes
  become output ancestry. Final DTB transformation occurs after firmware capture.
  The original formatter receipt and all produced scute origins reach inventory;
  cleanup remains required before publication.
- v211 full normal disposable-VM suite passes (233.33 guest seconds). New CLI cases
  cover implicit/explicit tools with/without retained carapace directives plus a
  missing-DTB failure after bootstrap. Firmware observes the selected tool files,
  embeds CARAPACE in PMI, and output reopening verifies that value equals the final
  published root and differs from the original tool root. Output has exactly one
  or four produced scutes and paired firmware; retained contents survive, temporary
  firmware files do not, and the formatter receipt is present. Failure publishes
  nothing and leaves no runtime attachment. Existing cached-base, firmware-only,
  transform, storage and lifecycle checks in the normal suite also pass.
- Host tests (87 library tests, integration suites, 52 doctests), strict root and
  fixture Clippy, formatting and diff checks pass. Evidence/source/executable hashes
  preserved under validation/rewrite-command-integration/evidence/v211. Full suite
  host observation timeout increased to 330 seconds for five additional real CLI
  builds; no production execution or cleanup deadline changed.
- Enabled networking/DNS is now the next major implementation gap: it is still
  explicitly rejected in pipeline and directive execution. General string-variable
  expansion audit, remaining stage/failure matrix and runtime acquisition faults,
  ORAS interoperability, three full M6 adherence passes, M7 qualification and M8
  retirement remain required. Runtime target directories are still author-supplied
  as recorded above. M6 stays active with zero clean adherence passes.

### M6 immutable resolver input

- Previous goal turn progressed in 667709a. Network source audit confirms that
  namespace entry currently creates an unattached network namespace and both
  pipeline/directive layers reject enabled execution. No NAT implementation is
  implied by the existing network-step plan/inventory schema.
- ResolverInput now captures the environment's selected regular file once during
  CapturedBuild construction for a plan with enabled steps. It pins with O_PATH,
  rejects nonregular objects before activation, reopens the pinned inode, checks
  capture length and retains immutable bytes without a source locator/handle.
  Debug redacts contents. Empty/non-UTF8 content is preserved without inventing DNS
  configuration. Inventory schema/serialization is unchanged and excludes this
  input as required. Offline plans do not open an otherwise supplied resolver path.
- Host tests prove capture through a symlink survives source replacement/removal;
  FIFO/socket/device/directory/missing/loop sources fail; empty contents remain
  empty; a committed enabled recipe requires a real readable resolver and retains
  the first capture across source changes. An offline plan ignores the subsequently
  missing resolver. All host tests (90 library tests, integration suites, 52
  doctests), strict Clippy, formatting and diff checks pass. Evidence/source hashes
  are in preservation/m6/network-input. No privileged VM/network test was necessary
  or claimed for this unprivileged capture change.
- Preserved network-policy and resolver-target/lifecycle experiments were read as
  precedent. Their earlier builder-endpoint filtering must not be copied: the final
  contract allows all reachable destinations. Resolver target handling must extend
  those experiments to absent/dangling paths, rollback and command quiescence.
- Required next integration: an owned private network namespace and scoped veth/
  route/NAT resources, setup acknowledgements before command admission, stateful
  reply-only ingress, confined read-only resolver injection from captured bytes,
  and removal/restoration after descendant quiescence. Preserve exclusive creation,
  failure ownership, original fixed deadlines and existing environment NIC/routes;
  do not infer cleanup authority merely from a reusable interface name/index.
  Actual enabled-step receipts must enter inventory only after successful execution.
- Safe API candidates inspected include rtnetlink link/namespace operations
  (https://docs.rs/rtnetlink/0.23.0/rtnetlink/struct.LinkMessageBuilder.html),
  nftables typed JSON (https://docs.rs/crate/nftables/0.6.3), and nftnl
  (https://docs.rs/crate/nftnl/0.9.4). No dependency/backend selection or privileged
  mutation was made here. Existing rustix exposes owned netlink sockets, and cached
  netlink-packet-core/route sources expose safe codecs; implementation must retain
  bounded acknowledgement handling and explicit resource ownership regardless of
  wrapper choice. Do not take library documentation as validation of the topology.
- Enabled network commands remain rejected. M6 is still active at zero clean
  adherence passes; remaining M6 implementation/qualification and M7/M8 are required.

### M6 bounded route requests and owned virtual interfaces

- Previous turn was a requested status report, not implementation progress. This
  turn implements native safe route netlink using pinned MIT packet codecs and
  existing rustix owned sockets. Each request has its own nonblocking socket,
  original absolute deadline, kernel-sender/sequence/framing validation and an
  explicit ACK. No synchronization sleeps or external networking commands.
- VethPair owns a protected control marker and pinned network namespace. Creation
  is exclusive; interfaces initially remain DOWN with no configured addresses.
  Cleanup checks creation-time locally administered MAC identifiers derived from
  the unique marker, never interface names alone. The two endpoints have distinct
  addresses. Only the trusted network controller may change their identifiers;
  these identifiers are resource tracking, not secrets or authentication against
  another privileged administrator.
- Failed acquisition is opaque and exposes cleanup only. Cleanup retains the
  original deadline and resource owner on failure. Abandonment retains the marker
  and links for dedicated-VM disposal rather than claiming successful teardown.
- Disposable VM v212-v215 preserved failures: IfAlias supplied during creation was
  absent from kernel replies. v214 diagnostics established that both aliases were
  absent although reciprocal peer indices were correct. v215 repeats v214 after
  an edit script failed before changing sources. v216 passes with explicit MAC
  identifiers: creation, initially DOWN interfaces, duplicate-name rejection
  without deleting the original pair, explicit removal of both links/marker, and
  expired-setup rollback. No host networking was modified.
- Full host tests and strict root/fixture Clippy checks cover the new code. Raw
  failed and successful logs/source provenance are retained under evidence/v212
  through v216. This is focused backend evidence, not a full M6 adherence pass.
- Still required: command network namespace handoff, routing and NAT, resolver
  mounting/scaffolding/cleanup, enabled-command admission/receipts, complete failure
  coverage and ORAS. Namespace-mismatch cleanup and abandoned-owner recovery also
  need runtime validation. M6 remains active with zero clean adherence passes.

### M6 network namespace ownership and peer handoff

- Previous turn progressed in 5ca1f27. Added a production network-namespace helper
  role using the existing bounded process identity/channel protocol. The creator
  unshares only NEWNET, stays in the supervisor user namespace, and waits for
  channel closure. The supervisor captures a distinct ns/net descriptor from the
  owned process, verifies unchanged creator user namespace, releases the creator
  and reaps it before returning NetworkNamespace. Failure retains process wait/
  cancellation authority through existing process errors. No readiness sleeps.
- VethPair::connect consumes the pair and NetworkNamespace, verifies ownership,
  and moves the peer through native netlink. ConnectedVeth owns both through
  teardown; opaque ConnectionFailure permits cleanup only and retains both the
  namespace and pair after failed handoff/cleanup. This does not itself prove
  command quiescence or authorize image commands. Links remain down/unconfigured.
- Disposable VM v217 passes distinct fresh namespace identities, unchanged
  supervisor namespace, absence of surviving creator children, peer disappearance
  from the original namespace, connected pair teardown and empty control marker
  directory. Earlier collision/expiry checks still pass. Full host tests and
  strict root/fixture Clippy pass; helper serialization roundtrip now includes the
  new role. Evidence/source hashes live in evidence/v217.
- Namespace creation and link handoff are now implemented. Command-helper input
  plumbing and entry, addresses/routes/NAT, resolver injection and cleanup,
  enabled-command receipts, remaining negative/failure cases, ORAS and three
  complete M6 reviews remain outstanding. M6 stays at zero adherence passes;
  M7/M8 and the full objective remain active.

### M6 command network entry transport

- Previous turn progressed in 8a3dbc6. CommandSetup now accepts an owned network
  namespace reference through a consuming constructor. Regular and gated setup
  packets have separate exact opcode/descriptor shapes for attached networking;
  root-entry packets remain network-descriptor-free. Receivers close rejected
  rights, require queued messages, reject truncation/wrong counts, and install
  received descriptors CLOEXEC. A transport test checks network identity/CLOEXEC.
- CommandNamespaces joins the supplied network while initial-user-namespace
  authority remains, rejects the current supervisor network, closes the handle,
  and then enters the command user namespace. Offline entry still unshares NEWNET.
  Neither path retains a namespace descriptor/restoration handle for image code.
  This remains trusted setup, not command admission or complete network policy.
- Focused disposable VM v218 passes both regular and gated transfers, selected
  network identity, mapped UID zero with parent UID 100000, and absence of an open
  network namespace descriptor after entry. Existing namespace creation/reaping,
  veth collision, peer movement, teardown and deadline cases still pass.
- Full normal offline regression VM v219 passes after these shared setup changes:
  command isolation/lifecycle, cancellation and failure cleanup, bootstrap and
  retained layering, firmware pair extraction, transforms and CLI publication.
  Host suite (92 library tests plus integrations and 52 doctests), strict root and
  fixture Clippy pass. Evidence/provenance are preserved under evidence/v218-v219.
- Enabled recipes remain rejected: supervisor-to-volume-helper plumbing, complete
  address/routing/NAT setup, resolver mounting/cleanup and actual network receipts
  are still required before admission. These results do not count as a full M6
  adherence pass. Remaining matrix/ORAS, three reviews, M7 and M8 remain required.

### M6 dual-stack link configuration

- Previous turn progressed in e25b9d5. Added immutable NetworkAddress descriptions,
  bounded native netlink address creation, link activation and default routes.
  LinkConfiguration verifies the private MAC identifier before changing a link,
  accepts at most one address per family and has canonical validated CBOR helper
  input. Host-side default gateways are rejected; no primary interface is changed.
- ConnectedVeth::configure returns a distinct ConfiguredVeth only after the trusted
  namespace helper exits successfully. The helper joins the owned namespace,
  applies peer addresses/routes and activates loopback. IPv6 private addresses
  explicitly suppress DAD because the allocated pair is isolated and owned; this
  avoids a timing sleep before use. Kernel rejection is still a build setup error.
- Configuration failures retain the connection and any live helper. Cleanup must
  terminate/reap the helper within its original cleanup cutoff before removing
  the pair/namespace. ConfiguredVeth does not certify NAT or command admission.
- Disposable VM v220 passes IPv4/IPv6 private-link UDP from mapped-root helpers
  through both regular and gated setup, default-route source selection via UDP
  connect (no upstream packet sent), and actual IPv4/IPv6 loopback send/receive.
  A deliberately unreachable gateway causes the configuration helper to fail;
  cleanup removes the links/control marker and leaves no creator/helper children.
  Existing collision, namespace, descriptor-closure and expiry checks pass.
- Host tests (93 library tests, integrations and 52 doctests) and strict root and
  fixture Clippy pass. The focused test does not repeat the full offline v219
  suite; those paths are unchanged. Source/log provenance is in evidence/v220.
- Required next: NAT/stateful reply-only ingress and forwarding ownership,
  supervisor-volume-helper plumbing, resolver mounting/cleanup and network
  receipts. Offline namespaces currently leave loopback down; activate it within
  the existing bounded setup before image execution and validate local-only
  communication. Required worker-timeout/partial-setup cases and ORAS/full M6
  reviews also remain. M6 is active at zero passes; M7/M8 remain required.

### M6 offline loopback under the original setup deadline

- Previous turn progressed in 7a02ad0. Extracted the existing native loopback
  operation into one shared internal object. Offline command namespace entry now
  activates loopback after creating its private network namespace and before
  privilege restriction; attached networks use their existing configured loopback.
- Regular/gated setup have explicit deadline entry methods. The production gated
  helper passes its original SetupCutoff deadline through; no renewal or sleep is
  introduced. Convenience entry APIs use a fixed internal 30-second ceiling.
  Expired entry rejects before namespace mutation (covered by a host unit test).
- Disposable VM v221 passes offline regular/gated entry with mapped UID zero:
  IPv4/IPv6 local send/receive works, /proc/net/dev contains only lo, and external
  IPv4/IPv6 UDP connect cannot select a route. Earlier attached-network descriptor,
  namespace, address/route, collision and failed-helper cleanup cases still pass.
- Host tests (94 library tests, integrations and 52 doctests), strict root/fixture
  Clippy and focused VM checks pass; source/log provenance is in evidence/v221.
  This is focused namespace evidence. Full offline regression v219 predates this
  loopback change; final integration review must cover the updated complete path.
- NAT/forwarding policy, DNS injection, supervisor-to-volume-helper integration,
  enabled-command receipts and remaining failure/ORAS cases remain required.
  M6 stays active at zero adherence passes; no M7/M8 advancement is claimed.

### M6 native nftables policy and local reply filtering

- Previous turn progressed in e6bc7c9. Selected pinned MIT-licensed
  netlink-packet-netfilter 0.4.0, compatible with the existing netlink-packet-core
  0.9 codec. No libnftnl or runtime nft executable dependency in production.
  Primary source/API: https://docs.rs/netlink-packet-netfilter/0.4.0/netlink_packet_netfilter/
  and the downloaded crate source (license/version captured with v222).
- Added bounded nonblocking nfnetlink batches on an owned CLOEXEC socket. Each
  operation has a unique sequence and requires its ACK; truncated, duplicate,
  unexpected and wrong-sequence messages fail. Unread old ACKs after failure
  cannot acknowledge a later transaction. Socket polling shares the existing
  absolute-deadline implementation. Host parser tests exercise these properties.
- FirewallTable uses kernel TableFlags::Owner and exclusive creation. Explicit
  close requires acknowledged deletion and retains the socket on failure. Socket
  ownership provides kernel cleanup on close; asynchronous abandonment/crash
  timing and failed-batch rollback still need dedicated runtime qualification.
- NatPolicy atomically submits the owner table, forward/output chains accepting
  established/related traffic toward the private interface, otherwise dropping
  inbound traffic, and a postrouting masquerade rule matching its ingress name.
  Scoped IPv6 neighbor-discovery exceptions require hop limit 255 and NS/NA type.
  Other interfaces/destinations are not filtered. Safe generic expression attrs
  cover ct/masq absent from the codec's typed enums; constants cite Linux UAPI.
- NatPolicy borrows ConfiguredVeth through close (including close failure), keeping
  explicit link teardown behind policy ownership. This is not command admission
  or forwarding-readiness proof. Production never changes the primary NIC here.
- Disposable VM v222 independently lists the resulting table with nft and passes
  IPv4/IPv6 request/reply from mapped-root namespace helpers. Ready listeners do
  not receive unsolicited local-origin packets while policy is installed; the
  same packets arrive after removal (positive control). Table-name collision is
  rejected without losing original policy; acknowledged deletion is observed.
  These are setup helpers, not the complete restricted image-command lifecycle.
- Host suite (95 library tests, integrations, 52 doctests), strict root/fixture
  Clippy and focused VM checks pass. Source/logs/lock provenance is in evidence/v222.
- Still required: forwarded upstream IPv4/IPv6 traffic and observed NAT source
  translation; unsolicited forwarded input; forwarding prerequisites/ownership
  without primary-NIC changes; crash and rejected-batch cases; DNS injection,
  lifecycle/plumbing, actual network receipts, remaining ORAS/matrix coverage and
  three complete M6 reviews. M6 remains active at zero passes; M7/M8 remain open.

### M6 forwarded IPv4/IPv6 UDP and TCP qualification

- Previous turn progressed in a88f2e1. Added a maintained disposable-VM fixture
  using production namespace creation, connected/configured veth ownership,
  mapped-user setup and NatPolicy. Command and upstream namespaces are entirely
  inside the NIC-less guest. No host network operation is performed.
- Fixture-only environment setup enables global IPv4/IPv6 forwarding before its
  links are created. An upstream helper binds all listeners before signaling
  readiness over a pipe; parent readiness is poll/deadline bounded, with no sleep.
  Each helper remains directly owned and deadline-waited. Ready command listeners
  make unsolicited UDP checks meaningful; receive timeouts are negative checks,
  not setup synchronization. TCP uses bounded connect/read/write and parent wait.
- v223 preserved a fixture error: nested pin directory was mode 0755; production
  pin validation rejected it before topology setup. Set the fixture directory to
  required 0700. v224 passes UDP; v225 adds TCP. Upstream asserts the actual IPv4
  and IPv6 masqueraded source addresses, replies reach the command, and unsolicited
  forwarded UDP cannot reach its listener while policy is installed.
- After acknowledged policy removal, fresh flows show the original source
  addresses and unsolicited UDP arrives. Explicit distinct UDP source ports avoid
  confusing retained conntrack NAT mappings with newly generated flows. Cleanup
  verifies all four link names, namespace pin/control directories and helper
  children are gone. Earlier local network cases also pass in each successful run.
- Strict fixture Clippy passes. Production source is unchanged this turn, so host
  tests were not redundantly rerun. Failed/successful source snapshots and logs are
  preserved in evidence/v223-v225. These are mapped-root helpers, not the final
  restricted image-command pipeline, and no DNS/attestation claim is inferred.
- Forwarding environment contract remains to be implemented: kernel documentation
  warns that changing IPv4 ip_forward resets other network parameters, and IPv6
  conf/all/forwarding changes interface host/router behavior. Production must
  honor the existing prohibition on primary-NIC changes; fixture sysctl writes
  are not a proposed production implementation. Primary reference:
  https://docs.kernel.org/networking/ip-sysctl.html
- Remaining: forwarding preflight/ownership; DNS mount lifecycle; complete command
  pipeline and actual network receipts; rejected-batch/crash/timeout coverage,
  unsolicited TCP coverage, ORAS, and three full M6 reviews. M6 remains active at
  zero passes. M7/M8 and full objective remain open.

### M6 confined resolver targets and read-only mount sources

- Added ResolverTarget: resolves /etc/resolv.conf against an exclusively owned
  command root, follows confined absolute/relative symlinks, creates missing
  ancestors/placeholders, and rejects loops, escapes, nonregular targets and
  command runtime mount locations (including aliases). Final openat2 resolution
  verifies the kernel's interpretation and inode identity. Files are inspected
  with O_PATH; FIFO/socket/device nodes are never activated.
- Temporary scaffold entries retain pinned parents and created inodes. Explicit
  reverse cleanup removes only those entries, refuses replacements/nonempty
  directories, and retains failures for retry. Abandonment requires discarding
  the writable layer. A mkdir followed by failed pinning remains an explicit
  unresolved cleanup obligation rather than claiming successful removal.
- Added ResolverSource on the build's existing tmpfs. A private generated name
  survives while borrowed ResolverMount views are prepared and attached. Source
  construction closes the writable descriptor; detached views are made read-only,
  nosuid/nodev/noexec before attachment. Explicit source cleanup retains its owner
  on failure. Resolver bytes remain excluded from inventory and no original
  environment pathname is reopened.
- Runtime evidence changed the implementation: an anonymous O_TMPFILE inode can
  be cloned as a detached mount, but move_mount rejected its attachment with
  ENOENT. v228/v229 preserve that failure and its operation diagnostics. The
  named source resolves it. v226/v227/v230 preserve fixture/compiler failures;
  none launched a successful runtime probe.
- v231/v232 verify exact non-UTF8 bytes after deleting the original input,
  read-only enforcement against root writes, restoration of existing contents,
  dangling /run symlink scaffold cleanup, and preservation of the image symlink.
  v232 additionally rejects a target pinned in a different mount namespace,
  before attachment. The existing network fixture still passes IPv4/IPv6 NAT,
  UDP inbound filtering/positive controls, TCP replies and teardown.
- Host tests additionally cover parent-directory replacement, replacement-inode
  cleanup refusal/retry, nonempty scaffolding, partial resolution failure,
  runtime aliases and incompatible target types. Full cargo tests/doctests and
  production/fixture strict Clippy pass. Evidence and source snapshots are in
  validation/rewrite-command-integration/evidence/v226-v232.
- This establishes the source/target/mount primitives, not complete DNS command
  integration: descriptor transport, setup ordering against idmapped volumes,
  command namespace access, quiescent cleanup, enabled recipe admission and
  actual network receipts remain. Forwarding preflight, remaining failure/matrix
  coverage, ORAS and three complete M6 reviews remain open. M6 is still active
  at zero adherence passes; M7/M8 and the full goal remain pending.

### M6 resolver transport into command namespaces

- CommandSetup now accepts a resolver only with a configured network namespace.
  Regular and gated setup packets carry the detached mount with exact new shapes
  (opcode 6/four descriptors and opcode 7/five descriptors). Receivers enforce
  CLOEXEC and reject malformed shapes, wrong objects, writable mounts and mounts
  lacking readonly/nosuid/nodev/noexec restrictions. No resolver bytes or source
  filename enter the packet.
- Before surrendering initial-user-namespace authority, trusted setup clones a
  private mount namespace and opens its assigned cwd there. It selects only an
  existing resolver target, installs the read-only view, closes all resolver/root
  handles and commits the namespace before ordinary mapped-root entry. Existing
  target selection cannot create scaffolding: ownership of temporary image files
  remains with the enclosing owner, outside command execution. The original
  setup cutoff is checked before this work and again before namespace entry.
- v234 proves both regular and gated transport through real mapped-root entry:
  exact contents are readable, writes fail, no resolver descriptor survives, the
  supervisor sees its unchanged empty placeholder after helper exit, and the
  scaffold owner removes it. Existing dual-stack NAT and offline-loopback cases
  also pass. This is namespace setup evidence; final pivot/root restriction and
  network-enabled CLI recipe integration are still outstanding.
- v233 preserves an overly narrow fixture assertion: mapped root received EACCES
  on write-open instead of EROFS because the supplied source owner is outside its
  mapping. The corrected assertion accepts either denial and rechecks contents;
  v232 already independently proved mount-enforced EROFS for initial root. The
  initial compile failure (thread-affine namespace errors cannot be boxed as
  Send+Sync io errors) and source snapshot are retained with this evidence too.
- Added host tests for writable resolver packet rejection, malformed packet FD
  closure and missing-target selection without creating image paths. Full cargo
  tests/doctests and production/fixture strict Clippy pass. v235 passes the full
  normal disposable-VM command/bootstrap/offline CLI regression (~245 seconds).
  Runs, source snapshots, logs and hashes are preserved in evidence/v233-v235.
- Next integration boundary: supervisor-to-volume-helper network/resolver
  transport, raw-root scaffold preparation and ownership through teardown,
  configured link/NAT lifecycle and forwarding preflight, enabled directive
  admission and actual network receipts. Remaining matrix/failure coverage,
  ORAS and three complete M6 adherence reviews still apply. M6 stays active at
  zero clean passes; this work does not advance M7/M8 or complete the goal.

### M6 owned NAT connection lifecycle

- NatPolicy now consumes ConfiguredVeth instead of borrowing it. This removes
  the self-reference obstacle to keeping network resources inside command
  teardown state. The policy exposes its namespace by shared reference, and
  acknowledged firewall removal returns the configured connection for link
  teardown. A compile-fail test rejects using the connection after assignment.
- Failed installation owns the connection and a fixed internal cleanup cutoff.
  Explicit cleanup removes only that connection. If cleanup fails, it retains
  the pending connection cleanup, initial installation diagnostics and original
  deadline; retries cannot renew the budget or manufacture a successful policy.
  Failed policy removal likewise retains the connection until removal succeeds.
- ConfiguredVeth::close_until allows the enclosing command teardown to pass its
  original cutoff through link deletion and namespace release rather than
  starting another independent timeout at that boundary.
- v236/v237 pass in disposable VMs: a colliding table prevents installation,
  failure retains live links until explicit cleanup, cleanup preserves the
  independently owned table, and expired installation removes its owned links
  without leaving a table. Both successful NAT traffic paths now use the owning
  API and return the connection before the no-policy positive controls.
  Existing IPv4/IPv6 UDP/TCP, resolver transport and namespace cleanup tests pass.
- Full cargo tests (including 53 compile-fail doctests) and production/fixture
  strict Clippy pass. Evidence, sources and hashes are preserved in
  validation/rewrite-command-integration/evidence/v236-v237. This ownership
  refactoring is not a completed command-network lifetime integration or an M6
  adherence pass. Supervisor step ownership/transport, scaffold lifecycle,
  forwarding readiness/address allocation, enabled recipes and network receipts
  remain next; M6 stays at zero, and M7/M8/full qualification remain outstanding.

### M6 network ownership through real command execution

- Integrated `CommandNetwork` into consuming `Step<Ready>` ownership using
  `NetworkedStorage`. It remains retained through command execution and domain
  quiescence; finalization closes NAT, connection and resolver before storage.
  Opaque cleanup failures retain both owners and the original teardown deadline.
- Added network volume handoffs for single-root and bootstrap layouts, with an
  independent raw tool-root view for resolver scaffold cleanup. The cleanup view
  stays in the trusted helper and is not exposed to image commands. Failed
  experiments v238–v241 are preserved alongside subsequent runs.
- v246 passed restricted image libc DNS and TCP against an isolated guest fixture,
  resolver write denial, successful scaffold restoration observed by an offline
  successor, nonzero command cleanup, and timeout network cleanup. Timeout kills
  the helper and leaves an empty mountpoint: runtime cleanup reports and retains
  that state. Fixture-only removal/retry is evidence, not production recovery.
  v243–v245 fixture failures and v242 success are also preserved. These results
  do not claim complete sandbox or platform qualification.
- Host tests pass, including exact network descriptor count rejection and ordered
  CLOEXEC transfer for both packet shapes. Root and fixture strict Clippy pass;
  the fixture's initial needless-reference lint failure is retained externally
  in the v247 evidence archive. Full normal VM regression v247 passed, including
  CLI pipelines and command lifetime/teardown checks. An additional compile-fail
  test proves a ready networked step cannot receive a replacement network; all
  54 doctests pass. These focused checks are not a full adherence review.
- CLI enabled-network admission remains rejected pending network allocation,
  environment readiness, directive integration and inventory receipts. Bootstrap
  network execution still needs runtime coverage. M6 remains at zero complete
  adherence passes; M7 and M8 remain outstanding.

### M6 network topology and private link selection

- Added bounded multipart netlink dumps with explicit DONE completion. A plain
  ACK cannot complete a dump, and interrupted or failed dumps reject the result.
  Existing mutation requests still require their own ACK.
- `NetworkTopology` reads links, assigned prefixes and IPv4/IPv6 routes across
  tables without changing the network. `LinkAllocation` selects unused interface
  names and private /30 and /126 prefixes, avoiding non-default routes and
  assigned prefixes. It skips occupied ranges rather than scanning every address;
  exhaustion fails. Default routes remain the upstream path.
- Allocation is a topology observation, not a reservation. Exclusive creation,
  setup ownership, forwarding readiness, enabled directive integration and
  inventory receipts remain required before CLI admission changes.
- Nineteen focused network tests pass, including actual read-only host capture,
  overlap/exhaustion/boundary cases and dump completion/interruption. Strict
  Clippy passes. Logs and source hashes are in evidence/network-topology-1.
  M6 remains at zero full adherence passes; this is not platform qualification.

### M6 enabled-network CLI integration (validation in progress)

- Added `CommandNetwork::prepare` to compose topology selection, namespace,
  private links, dual-stack configuration, NAT and resolver source under one
  setup cutoff. `NetworkSetupFailure` retains partial resources and creator
  processes for explicit bounded cleanup. Configuration rollback now preserves
  its original deadline through link removal instead of starting another one.
- The directive runner now supports captured `CommandNetworking`. It attaches
  successful setup to the ready step; its opaque network-setup rejection keeps
  the ready command blocked until any separately owned network creator is reaped.
  Only then may command teardown perform its process-wide child drain. Failed
  cleanup cannot expose a ready step or an accepted command receipt.
- Bootstrap, carapace and firmware CLI paths use the new runner. Successful
  inventory construction records their authored enabled-step IDs after all
  stages complete; resolver bytes and source paths remain excluded.
- Current setup requires environment IPv4 and IPv6 forwarding enabled and does
  not write sysctls or change the primary NIC. This implementation prerequisite
  still needs deployment/qualification review; it is not a new portability claim.
- Full VM regression v248 passed. Its new case exercises TCP and resolver
  injection in all three CLI stages, checks network inventory and teardown, and
  rejects a missing resolver argument. Existing offline pipelines and lifecycle
  cases also passed. Host tests, all 54 doctests, and strict root/fixture Clippy
  pass. Source, compiler logs (including the initial ownership compile failure),
  serial output and provenance are preserved in evidence/v248.
- Enabled CLI networking is now implemented and demonstrated in this disposable
  x86-64 VM. Forwarded dual-stack traffic was qualified separately by the earlier
  network fixture; this CLI case uses a guest-local TCP service. Further setup
  fault/cleanup-expiry tests, network command failure publication checks, ORAS,
  and three complete M6 adherence passes remain. M6 stays at zero passes.

### M6 enabled-network rejection and independent OCI copy (in progress)

- Added CLI negative cases for disabled IPv4 forwarding, disabled IPv6 forwarding,
  and an admitted network command exiting 37. They check no output publication,
  command admission markers, scoped link/table removal and runtime cleanup.
- v249 preserved a failed assertion: both forwarding cases passed; the nonzero
  command produced a raw Debug wait status (9472) instead of readable exit code
  37. Publication/runtime assertions for that case had not yet executed. Added
  Display diagnostics for command outcomes and a focused exit-code test; the
  original failure and compiler/test logs remain evidence, not a passing run.
- Added required ORAS copy checks to each successful output in the normal VM
  pipeline suite. ORAS 1.3.5 uses the preserved legacy CI release URL and archive
  SHA256 f48a73ea6d026a862c3919fc773da9a8c86fc259c2c737d5baed3b8b9614ad22;
  the harness verifies binary SHA256
  7c43d74f5f64c94932b31c76103bb7f48af837fe8aa5316fba7968b6c523350e.
  ORAS selects the manifest by digest, copies between OCI layouts in the guest,
  and every referenced blob must have identical bytes and digest. The disposable
  VM has no external NIC. The successful network inventory is also exported
  through the serial transcript into a separate CBOR evidence file.
- v250 passed these checks and the full regression. Both forwarding preflight
  failures and the admitted nonzero command rejected publication and left no
  scoped network or runtime resources. ORAS copied all 12 successful pipeline
  output variants with byte-identical blobs. Focused outcome tests and strict
  root/fixture Clippy pass. The CBOR inventory, ORAS version and artifact hashes,
  source snapshot, logs and serial are preserved in evidence/v250.
- This establishes local OCI-layout interoperability for these emitted outputs,
  not registry/platform qualification. Composite partial-setup and cleanup-expiry
  coverage, remaining requirements audit and three complete M6 adherence passes
  remain. M6 stays at zero passes.

### M6 composite network failures and initial adherence audit

- v251 passes the complete network suite plus failure after namespace creation,
  NAT installation collision preserving an existing foreign table, and repeated
  expired teardown retries retaining their resource owner. The expiry probe is
  terminal and explicitly relies on dedicated-VM disposal; it claims no cleanup.
- v252 adds successful IPv4/IPv6 unsolicited TCP rejection while established
  outbound traffic receives replies. Removing the policy permits the same
  inbound connection and payload. Listener readiness is established by the
  existing outbound request handshake; only the negative observation is timed.
  Strict fixture Clippy passes. Both runs and sources/provenance are preserved.
- Started preservation/MILESTONE-6-ADHERENCE.md with the requirements/evidence
  map. Source review found R1: topology observation is not serialized with private
  subnet assignment, so concurrent setups with different link names may choose
  overlapping addresses. Fix requires an owned namespace-scoped reservation or
  serialization guard, bounded/fail-fast acquisition and VM validation. No clean
  pass has started; M6 remains zero and later milestones remain pending.

### M6 adherence R1 correction: namespace-scoped allocation lease

- `AllocationLease` holds a nonblocking exclusive flock on the current network
  namespace inode from before topology observation through setup success/failure.
  Its owned CLOEXEC file closes by RAII; competing setup fails before mutation.
  No shared lockfile, random-subnet assumption, retry loop or sleep was added.
- v253 establishes namespace flock behavior. v254 exercises the production guard,
  competing process and mount namespace, independence across network namespaces,
  failure release, and distinct prefixes for two simultaneously live networks.
  Teardown of the second preserves the first. Full network regression and strict
  root/fixture Clippy pass. Both runs are preserved. R1 is addressed; these focused
  checks do not constitute a full clean adherence pass.
- Reconciled DESIGN section 11's explicitly proposed enclosing-layout illustration
  with the user's OCI-directory requirement: `--output` itself contains OCI
  entrypoints, blobs and inventory.cbor. This documents the current ORAS-verified
  representation; no implementation, wire-format or attestation scope changed.
- M6 remains zero clean passes. Source/design/style/ownership review continues.

### M6 adherence R2: CLI log stream contract

- Review traced inherited helper stdout to recipe/formatter output, contrary to
  DESIGN section 2's stderr logging contract. The shared helper factory now
  duplicates stderr as stdout before spawn; private control transport is unchanged.
- Extended the full CLI matrix to require empty stdout on success and failure,
  successful stage markers on stderr and failed-command admission on stderr.
  Strict root and fixture Clippy pass. Full VM/ORAS regression v255 passed;
  sources, logs and provenance are preserved in evidence/v255. This focused
  correction evidence is not a complete adherence pass. M6 remains zero.
- Continued source inspection of firmware ancestry/extraction, confined COPY,
  DTB transformation and output staging. Review coverage and a remaining firmware
  diagnostic-path question are recorded in MILESTONE-6-ADHERENCE.md.

### M6 adherence R3 and environment prerequisites

- Corrected firmware extraction error masking when cleanup also fails. Both
  diagnostics now survive; finalization still requires extraction and cleanup
  success. Original consuming cleanup order and deadline are retained.
- Strict Clippy and all-target tests passed; source/log evidence and the initial
  borrow-check failure are preserved in evidence/firmware-diagnostics. No new
  simultaneous kernel fault-injection claim.
- Documented the existing dual-stack forwarding prerequisites; v250 supplies
  refusal evidence for each disabled setting. Conglobate does not mutate them.
- Continued atomic publication and firmware assembly audit, recorded in the M6
  adherence ledger. No full clean review claimed; M6 remains 0/3.

### M6 first complete clean adherence pass

- Previous goal turn fixed R2/R3. Committed the previously untracked evidence
  archives in ec81986 so preservation does not depend on local untracked files.
- Completed integrated requirements/ownership review on frozen scope
  preservation/m6-review/scope-v1.json. Covered coordinator combinations, confined
  COPY and paired firmware extraction, transforms/firmware contracts, networking
  ownership/transport/DNS, deterministic inventory and verified atomic publication,
  Rust style and evidence/qualification boundaries. No further finding.
- Current 54 doctests passed; existing current host tests, strict Clippy and v255
  plus focused network evidence were inspected rather than gratuitously rerun.
- Clean streak 1/3. M6 remains gated; two complete consecutive reviews are required.
  M7/M8 remain pending. Detailed pass record: preservation/MILESTONE-6-ADHERENCE.md.

### M6 second complete clean adherence pass

- Previous turn completed pass 1. Verified every one of the 180 frozen-scope hashes
  unchanged, then reviewed backward from publication through coordinator failures,
  command/storage/network finalization, policy/pin removal and input provenance.
- Challenged evidence against fixture assertions and actual archived source;
  v255 differs only in the already reviewed R3 diagnostic/doc correction. Verified
  396 repository-retained provenance entries across v250–v255 without mismatch.
  External artifact durability remains explicitly M8 work.
- No new finding or source change. Rust ownership/style and complete requirements
  remain in scope; evidence is not overstated as independent/platform qualification.
- Clean streak 2/3; one full review remains. M6 is not yet accepted; M7/M8 pending.

### M6 third clean pass and acceptance

- Previous turn completed pass 2. Reverified all 180 frozen file hashes and
  completed public-authority/acceptance review against stage/output combinations,
  input roles, immutable plans, sealed typestate, cleanup and publication gates.
- No new finding or implementation change. Detailed complete pass is recorded in
  preservation/MILESTONE-6-ADHERENCE.md; M6 accepted with 3/3 consecutive clean
  self-reviews on preservation/m6-review/scope-v1.json.
- M7 now begins with integrated critical review, distro builds/boots and CI/platform
  qualification. M8 preservation/retirement remains pending; full goal is active.

### M7 qualification inventory

- Added M7-QUALIFICATION.md with the complete review/distro/architecture/platform/CI
  matrix, observed environment and concrete next actions. No qualification passes
  inferred from installed tools or existing sibling mkosi output.
- Located the user's Claude review command, sibling Fedora reference pipeline,
  local artifact directory and preserved CI workflow. Active CI is absent; the
  old privileged sudo/unshare lane must be replaced with disposable-VM execution.
- No sibling source modifications, privileged host operations or publication.
  M7 is active at 0/3; all original M7/M8 requirements remain outstanding.

### M7 independent review launched

- Started the user's custom Claude Code /review workflow against active source,
  with supporting tests/designs and explicit trust/style constraints. Read-only;
  no privileged host tests or source modifications permitted. Persistent session
  and exact invocation are in preservation/m7-review; exec session 81989 remains
  live at the latest poll. Continue that process, not a replacement run.
- Some compound mechanical checks were denied by reviewer command permissions;
  they remain unperformed, not passing. Parent formatting check passed (exit 0).
- User reiterated that findings must be critically evaluated and negotiated to
  source/test-grounded consensus. The disposition process is recorded in the
  review README; no finding or review verdict is accepted yet.
- Read-only Fedora cache inspection verified four component content identities.
  The selected image is a direct manifest; fixture index preparation, chain
  verification and conglobate distro build/boot are still required.


### M7 review remediation: shared storage finalization

- Consolidated the single-volume and bootstrap finalization machines using the
  existing sealed storage traits; retained typed aliases and consuming retries.
- Original deadlines, concrete storage retention, diagnostics history and primary
  command outcomes remain intact. Snapshot validation belongs to StorageLayout.
- 198 rootless tests, 54 doctests, strict stable Clippy and formatting pass.
  Full disposable-VM v261 passes; source, logs and provenance are retained.
- Independent review reconciliation remains active. Follow-up review must assess
  this refactor and evidence; no consensus or M7 adherence pass is claimed.
  M7 stays at 0/3; distro, platform, hosted CI and M8 retirement remain outstanding.

Registration-free M2 revision: three frozen-scope parent adherence passes are
recorded in `preservation/m7-review/policy-revision-adherence.md`, with terminal
Claude consensus and hashed design scope. M4–M6 remain reopened at zero.


### Registration-free implementation: namespace ownership

- Removed policy-identity namespace bind pins, their control records and cleanup
  states. UserNamespace<Mapped> owns the descriptor required for idmapped mounts
  and command setup; step ownership retains it through descendant quiescence.
  Cgroup control records, mount ownership and cleanup remain in place.
- Retired source and namespace-record tests are preserved in the hashed
  namespace-pins-24ccd81 archive under preservation/scoped-policy-522011a.
- 192 rootless tests, 50 doctests and strict all-target Clippy pass. Three
  namespace-record tests and one obsolete pin compile-fail example retired;
  the ID-range boundary test is retained. Initial failed checks are preserved.
- Native amd64 disposable-VM positive test executes the recipe, exports the OCI
  artifact and verifies its hashes; missing-policy negative test rejects before
  recipe execution with the specific labeling diagnostic and complete cleanup.
  Evidence: preservation/m7-review/label-unpinned-{on,off}-v1. External large VM
  assets remain under /var/tmp and still need durable recovery before M8.
- These are narrow single-root regressions, not milestone adherence reviews.
  Bootstrap two-root, broad failure/isolation migration, independent review and
  three clean adherence passes remain outstanding. M4–M6 stay reopened at 0/3.


### Registration-free bootstrap regression and design correction

- Actual CLI bootstrap passes in native amd64 disposable VM with fixed external
  labeling policy: trusted ext4 formatting on dm-zero snapshot, two assigned
  roots, restricted pre-admission anonymous label probes, two command steps,
  numeric ownership/mode persistence across remount, clean sealing and export.
- Independent assessment verifies canonical CBOR, bootstrap-tools input role,
  one produced scute, no inherited tool firmware or scutes, exact OCI descriptor
  sizes/hashes and blob closure, and measured formatter runtime/configuration.
  Evidence: preservation/m7-review/bootstrap-unpinned-v1; maintained recipe and
  assessment: tests/distro/bootstrap-labeling. This minimal filesystem is not a
  distro boot qualification and does not replace broad failure/isolation tests.
- Found stale normative bootstrap wording requiring per-filesystem LSM scopes
  in DESIGN.md 6.1; corrected to the fixed environment exception, both-root
  admission checks and inode-reachability boundary already specified in section
  9 and SANDBOX-DESIGN.txt 4. Architecture review scope changed: the prior M2
  passes remain historical evidence; revised M2 streak resets to 0/3 pending
  renewed adherence review. M4–M6 remain 0/3; M7/M8 incomplete.
- Broader legacy command fixture still references retired policy/pin APIs and
  requires explicit coverage migration. It is not presently a runnable current
  regression suite. No historical failure or validation evidence was removed.


### Registration-free architecture reconciliation in progress

- Removed conflicting scoped-policy transitions and stale pre-integration status
  from active ARCHITECTURE.md. Preserved the full original architecture and M4
  integration review with SHA-256 records under
  preservation/registration-free-architecture-c1073b9. Historical dependency
  appendices remain recoverable there; current source and lockfiles govern.
- Corrected source comments for descriptor release, storage quiescence and
  containment. No production behavior changed in this documentation pass.
- Resumed the existing custom Claude review session for the source changes from
  522011a through c1073b9 plus architecture reconciliation. Review invocation and
  raw stream are under preservation/m7-review/registration-free-implementation-review.
  Findings require parent source-grounded evaluation and consensus. No clean
  adherence pass or milestone acceptance is claimed; M2/M4–M6 remain 0/3.


### Registration-free CLI error-path regression: timeout failure found

- Maintained tests/distro/failures recipes and assessor exercise ordinary exit 7
  and a finite one-second recipe timeout. Trusted VM runner explicitly checks
  that failed builds have no published output directory.
- Nonzero case passes command error preservation, complete command/runtime
  cleanup and no publication. Timeout case starts the command, reaches the
  execution deadline and rejects publication, but fails final runtime cleanup
  with Directory not empty. This is a failed regression, not accepted cleanup.
  Evidence is preserved in preservation/m7-review/cli-{nonzero,timeout}-v1.
- Working hypothesis from source: receiver creates MountPoint under the shared
  runtime volumes directory; cgroup cancellation kills the helper before its
  temporary directory destructor can unlink that point. Namespace teardown
  removes private mounts but not shared tmpfs directory entries. Runtime::close
  correctly refuses to remove a nonempty volumes directory. Confirm residual
  ownership and implement cancellation-safe attachment storage; do not paper
  over this with sleeps, recursive deletion or relaxed cleanup/publication.
- Claude implementation review still running in the existing session (tool
  session 78027 at this checkpoint). Its scope predates this new failure test;
  present the failure evidence during source-grounded consensus reconciliation.
  All reopened gates remain 0/3. No complete sandbox qualification is claimed.


### Cancellation-safe receiver attachment

- Replaced helper-created shared temporary mountpoint with an overlay on the
  existing supervisor-owned runtime volumes directory in the helper's private
  mount namespace. MountPoint opens a protected root-owned tmpfs target and
  parent; attach verifies the private namespace is current. No extra tmpfs,
  shared-directory creation, destructor-dependent unlink or recursive sweep.
- Explicit normal unmount remains fallible; cancellation removes private mounts
  through namespace death while the supervisor retains storage through drain.
  Removed the obsolete post-unlink failure variant and made recovery private.
- Timeout regression now passes cleanup and no-publication checks (cli-timeout-v2).
  Actual two-root bootstrap/remount/sealing and independent graph/inventory
  assessment pass (bootstrap-attachment-v2); missing-policy rejection retains
  its diagnostic, completes cleanup and publishes nothing (label-attachment-off-v1).
  Prior timeout failure remains preserved. VM source snapshots record exact
  versions; timeout run precedes the private recovery-type cleanup, bootstrap
  precedes its visibility-only warning correction, final negative uses both.
- 192 rootless tests and 50 doctests pass; final strict all-target Clippy passes.
  Evidence and initial compiler warning are retained. No adherence pass inferred.
  M2/M4–M6 remain 0/3. Claude review session 78027 remains active at this
  checkpoint; its follow-up must include the timeout failure and this fix.


### Maintained CLI confinement observations

- Added tests/distro/confinement recipe/assessor using actual production CLI with
  the global environment labeling exception. Native amd64 ordinary VM v3 passes
  PID-1 parentage/access denial, live-FD allowlist and pipe/null stdio, five
  capability sets without SYS_ADMIN, no_new_privs and seccomp mode, nonzero ID
  maps, private device/devpts observations, read-only proc/sysfs, loopback-only
  networking and fresh shm alongside persistent filesystem data across steps.
  Build/export and inventory's disabled-network record also pass.
- v1 failed before its first checkpoint; tracing in v2 located a fourth proc-FD
  glob entry. v3 confirms this entry was already closed (enumeration descriptor).
  Corrected the test to ignore only closed entries, retaining rejection of every
  live FD beyond 0/1/2. No production behavior changed. All failed attempts and
  the successful source/logs/inventory are preserved under confinement-v1/v2/v3.
- These observations migrate a subset of old harness checks, not its broad
  fault-injection, metadata, syscall or platform coverage. No complete sandbox
  claim or adherence pass; reopened gates remain 0/3.
- Existing Claude review remains live under session 78027. Prepared follow-up
  prompt records timeout failure/fix and new evidence; send only after the
  current invocation is terminal, preserving its session and findings.


### Named pipes and background-child cleanup

- Maintained CLI descendant fixture initially failed before teardown because the
  syscall filter denied mkfifo. Named pipes are ordinary filesystem objects,
  distinct from unsupported persistent character/block devices. Permit only
  S_IFIFO in mknodat mode argument 2 (both supported ABIs) and legacy amd64
  mknod mode argument 1. No new capability or unrestricted syscall is allowed.
  DESIGN/SANDBOX-DESIGN clarify the distinction; gates remain reopened.
- Native amd64 v5 passes FIFO creation, character/block creation denial, and
  cleanup of an unwaited background child. The child holds a read/write FIFO FD
  before readiness acknowledgment; next-step nonblocking writer-open gets ENXIO
  and no child output exists. No sleeps establish ordering. Actual CLI build,
  clean remount and artifact export succeed. This is one concrete descendant
  case, not comprehensive helper-death or syscall/architecture qualification.
- Preserved v1 incorrect host-cache-path construction failure, v2/v3 FIFO denial,
  v4 passing but weaker readiness ordering, and v5 strengthened test. Evidence
  and source snapshots are in preservation/m7-review/descendants-v1..v5.
  192 rootless tests, 50 doctests and strict all-target Clippy pass (fifo-static).
- Existing Claude review remains running (session 78027); completed subreviews
  have been read provisionally. Accept differential second-root admission and
  command-level xattr coverage gaps; challenge assertion that geometry checks
  merely mirror implementation, since DESIGN 6.1 mandates that geometry and
  formatter capacity. Full evaluation/consensus awaits terminal synthesis and
  follow-up review of current fixes. No adherence passes counted.


### Provisional review remediation and command-level label validation

- Removed unused UserNamespace::idmap/NamespaceViewFailure public wrapper. The
  production handoff already maps received Mount owners directly. Kept namespace
  descriptor ownership and mapped-state constraints; no runtime behavior change.
- Added rejection of every unknown assigned-root tag, and indefinite-length CBOR
  array / embedded-NUL domain-record cases. 193 rootless tests, 50 doctests and
  strict all-target Clippy pass; review-boundaries-static preserves logs.
- Maintained missing-policy assessor reproduces rejection of the historical
  off-v1 failure and acceptance of off-v2/latest clean negative using only
  repository-preserved serial/status files. New label-assessment.json files are
  derived evidence; original records remain unchanged.
- Actual native amd64 CLI bootstrap label-operations-v1 passes command-level
  foreign-label set/read/remove/re-add on both roots, persistence across per-step
  remount, exact-name security/trusted negatives and read-only /sys EROFS. Uses
  verified local prior Fedora output as tool image, no network/downloads. Output
  seals/exports with one produced scute. Tool-input provenance and complete small
  evidence preserved; large external artifacts still need durable M8 recovery.
- This closes command-level xattr coverage for these operations, not the separate
  differential second-root pre-admission probe test. Full distro/architecture/
  confidential-platform qualification and legacy fault migration remain open.
- Claude review invocation remains live in session 78027; follow-up prompt updated
  with source-grounded remediation and disagreements for terminal reconciliation.
  No new adherence pass or milestone acceptance is claimed.


### Differential output-filesystem admission validation

- Added VM-only test environment fault, separate from unchanged production
  conglobate and normal exact-name labeling exception. Additional inode_setxattr
  hook matches the anonymous probe value, atomically records the first device,
  allows that device and denies a different device. No userspace registration,
  binary test hook, map update API, extra application dependency or capability.
  Trusted post-build observer reports counts; sources/objects are measured by
  fixture input hashing. Loader never executes on the host.
- Single-root control succeeds with allowed=1/denied=0. Bootstrap negative records
  allowed=1/denied=1, fails before recipe execution with label EPERM, completes
  cleanup and publishes nothing. Normal two-root positive is already preserved.
  This test would reject a root-only probe implementation: no denied output write
  and successful build/admission violate negative assertions. This is targeted
  native amd64 coverage, not all sandbox/platform qualification.
- Maintained fixture/assessor: tests/distro/labeling-fault. Evidence:
  preservation/m7-review/second-root-{control,negative}-v1. Negative build runner
  exits 1 as expected; separate assessment passes. Python parsing and C/BPF
  compilation with warnings denied pass; no application Rust changed this turn.
- Updated follow-up review prompt with proposed closure of differential coverage
  finding. Existing review session 78027 is still live; no replacement launched,
  consensus or adherence pass inferred. M2/M4–M6 remain 0/3; full goal incomplete.

### Registration-free review consensus and receiver startup cleanup

- Completed initial Claude review and focused source-grounded reconciliation;
  preserved original streams (gzip), final findings and explicit retractions in
  registration-free-implementation-{review,consensus}. Reviewer withdrew claims
  about lost tmpfs validation, stale test counts and command-level label tests
  detecting missing admission probes. Differential probe coverage accepted;
  additional device-identity guard reclassified as optional defense-in-depth.
- Fixed the remaining admitted-channel lifetime race: ReceiverStartup owns the
  running mount, diagnostic relay and original setup deadline. Healthy supervisor
  channel stays in the caller through start-gate failure cleanup. Earlier errors
  close the leaf gate before waiting. No sleeps, policy registration, new timeout
  or renewed deadline. Added root and operation context to label probe failures.
- receiver-root-negative-v1: differential output-root denial, /mnt write context,
  command not admitted, complete cleanup, no output; maintained assessor passes.
  receiver-root-positive-v1: actual CLI bootstrap and remount pass, exported graph,
  formatter receipt and canonical inventory verified. Native amd64 ordinary VM
  only. Small artifacts preserved; large artifacts explicitly not durably archived.
- Receiver refactor passes 193 rootless tests and 50 doctests; final diagnostic
  edits pass strict all-target Clippy. Logs: receiver-cleanup-static. Exact
  post-admission gate-send failure has NOT been deterministically injected; the
  readiness failure exercises shared cleanup but does not establish that branch.
- Focused review of final dirty source is running (session 45045), with the above
  coverage gap disclosed. No whole-milestone adherence pass claimed. M2/M4–M6
  remain 0/3; legacy fault migration, durable M8 recovery and full distro/platform
  qualification remain outstanding. Prior turn was a status-only response; this
  turn resumes concrete remediation and records new evidence.

### Focused receiver review resolved; execution-gate failure regressions

- Focused Claude invocation 45045 is terminal exit 0. Closed both source findings
  with no new defects. Original stream, result and parent evaluation preserved in
  receiver-cleanup-review. Parent qualifies overly broad cleanup wording: wait
  failure retains the running mount and drops the unjoined relay in an exit-only
  helper; cancellation remains supervisor-owned. Protocol failure need not mean
  a physically broken socket, but closing that channel still fails closed.
- Added deterministic socket tests for peer loss after readiness and expired
  start deadline. The latter also verifies EOF without start authorization at the
  ready child. No sleeps or production injection mechanism. Exact receiver-level
  post-admission failure/cleanup ordering remains an explicit coverage gap.
- Final suite: 195 rootless tests and 50 doctests pass; updated README count.
  Evidence: receiver-gate-static. Legacy command-integration still references
  removed registration APIs; its broader meaningful cases must be migrated before
  retirement. No adherence pass or milestone acceptance claimed.

### Architecture adherence restart: documentation findings

- Compared registration-free normative design, architecture ownership/transition
  descriptions and current source boundaries. Found summary table still claiming
  M2 3/3 despite its later reset; corrected to reopened 0/3. Historical passes
  remain intact. Found proposed module/type names presented as current ownership;
  replaced with actual source owners and clarified pipeline/error descriptions.
- Review attempt stopped on findings, not counted as clean. Scope hashes and
  observations: preservation/m7-review/architecture-reconciliation-303f765.
  No production change or retirement. Full architecture adherence sequence still
  required; no dependent milestone advancement authorized by this correction.

### Architecture requirements/evidence review: stale validation requirement

- Found DESIGN section 12 still requesting scoped LSM authorization and denial
  elsewhere, and section 13 still naming a multi-filesystem LSM lifecycle.
  Corrected these to the fixed exception, both-root restricted admission and
  inode/descriptor/read-only confinement. Historical scoped probe descriptions
  remain historical evidence. Replaced stale version-two implementation-status
  wording with the normative requirement to emit/validate version two.
- Added architecture traceability anchors for inputs, stages, ownership,
  formatting, labels, isolation, teardown, COPY/firmware, transformations,
  networking, publication and distro/platform/preservation. Explicitly separate
  current rootless/VM checks from historical fixtures and outstanding coverage.
- Findings reset/retain M2 at 0/3; no implementation gate advanced. Review scope,
  source observations and corrected hashes are preserved under
  preservation/m7-review/architecture-traceability-9c412ef. No production change.

### Full architecture review in progress at fc0ad91

- Previous turn made documentation/traceability corrections. Started full custom
  Claude follow-up review on frozen architecture/design scope (session 81625,
  verified live), read-only with no privileged operations. Scope hashes and parent
  preliminary findings are under preservation/m7-review/architecture-full-review.
- Parent source trace found an outcome-owner mismatch: architecture attributes
  command outcome to Step<Quiescent>, but source retains helper status there and
  separately carries CommandOutcome through CommandResult/ExecutionOutcome.
  Acceptance checks the latter after finalization. Reconcile prose, not sound
  source. Also reconcile no-VM integration wording with disposable-VM privileged
  qualification. Leave reviewed documents frozen until terminal synthesis.
- Findings mean no clean pass; M2 remains 0/3. No dependent milestone advanced.

### Full architecture findings corrected; consensus pending

- Full Claude architecture review (81625) terminated exit 0. It verified current
  type names, state flows, filesystem schema and test anchors, but found the
  no-VM deployment wording contradicts the dedicated-VM safety premise. Fixed
  DESIGN 1.1 to distinguish code dependencies from deployment and rootless from
  privileged tests. Added an in-place superseded marker to historical M2 acceptance.
- Corrected parent's substantive finding: Step carries helper status; the recipe
  outcome is independently preserved through CommandResult/ExecutionOutcome and
  checked after finalization. Cleanup/helper success is not command success.
- Challenged reviewer F3: crate-private descriptor access is a valid compile-time
  API boundary, and the original text states an obligation, not a guarantee about
  arbitrary internal trusted code. Added mechanism clarification; requested a
  concrete public escape or retraction. Descriptor source citation was incorrect.
- Original full review preserved with compressed stream and result. Focused
  consensus invocation 82339 running; prompt records exact dispositions and
  source references. No clean pass counted while findings reconciliation remains.
  M2 stays 0/3; M4-M8 are not accepted by this architectural review.


### Corrected architecture consensus and first clean pass

- Claude consensus 82339 terminated exit 0: F1 fixed, F2 severity downgraded,
  F3 retracted (visibility is an appropriate Rust API boundary), parent outcome
  ownership correction confirmed. No disputed architecture findings remain.
  Full terminal stream/result preserved in architecture-consensus.
- Parent requirements-first full architecture review is clean, 1/3, on frozen
  2e320cd document/source-anchor hashes. Detailed category observations and
  preservation verification: preservation/m7-review/architecture-adherence-2e320cd.
  This is a self-review, not an independent pass or repeated test run.
- M2 is not yet accepted; two full clean passes remain. M4-M8 implementation,
  regression migration, distro/platform and durable recovery requirements remain.


### Corrected architecture second clean pass

- Failure/publication-first parent self-review clean, 2/3. Traced raw staging
  versus CompletedImage, inventory/context cleanup, command outcome versus helper
  status, finalization/drain, network cleanup and partial namespace acquisition.
  Also reviewed schema, module/style obligations, evidence and preservation scope.
- Frozen 2e320cd hashes verified unchanged before and after. Detailed record in
  architecture-adherence-2e320cd/passes.md. No test run counted as a pass; no source
  or normative document changed. One full pass remains; M4-M8 are not accepted.


### Corrected architecture third clean pass and M2 acceptance

- Evidence/claims-first full parent self-review clean, 3/3 on unchanged 2e320cd
  scope. Read maintained assessors and recipe assertions against claimed limits,
  then rechecked requirements, modules, style, ownership/recovery, schemas and
  preservation. Detailed observations in architecture-adherence-2e320cd/passes.md.
- M2 accepted. This restores the architecture gate only; M4-M6 stay 0/3,
  M7 incomplete and M8 pending. Next work is meaningful legacy lifecycle fault
  coverage migration under the fixed environment policy, including the receiver
  post-admission failure case; no historical passing run substitutes for reruns.

### Maintained lifecycle harness migration started

- With M2 accepted, checked historical command-integration against current API:
  64 errors, principally removed policy/pin owners. Preserved full compiler log
  and the check-generated Cargo.lock diff, then restored that generated lock
  mutation so historical input remains unchanged. No privileged host execution.
- Added tests/lifecycle safe-Rust fixture binary, initially extracting formatter
  deadline/cancel/reap, exhausted-snapshot rejection and terminal drop-retention
  cases. Exact formatter test body is unchanged from 5ee7018 and hash-verified;
  original source/evidence retained. Entrypoint handles trusted helper re-exec and
  requires the disposable guest marker. Marker is not a security boundary.
- Current API check and strict all-target Clippy pass. Evidence/provenance:
  preservation/m7-review/lifecycle-migration-start. New runner/fresh guest runs
  still required; compile success does not qualify lifecycle behavior. Remaining
  command/network and broader fault cases are not removed or claimed covered.
- Network fixtures formerly read namespace handles through policy-specific pins.
  Do not restore pins or widen raw-descriptor exposure solely for tests; adapt
  those fixtures to current production ownership interfaces. M4 remains 0/3.

### Maintained formatter guest runner and fresh cancellation/exhaustion pass

- Added tests/lifecycle/run.py: rootless local build/packaging, disconnected
  native amd64/KVM guest, trusted formatter runtime manifests, raw supervisor
  status disk and required case markers plus guest poweroff. Captures actual
  source, file hashes, kernel identity and command. No host mount/BPF/device work.
- maintained-formatter-v1 passes fixed setup deadline cancellation and child
  reaping, exhausted snapshot rejection, explicit resource release, exit zero
  and VM poweroff. Expected mke2fs I/O error is the exhaustion negative, not a
  successful formatted output. Small complete evidence preserved; large assets
  explicitly not durably archived. This subset needs no labeling exception.
- Drop-wait retention run is live (42448); not yet claimed passing. Immediate
  drop-running assessment must not require child scheduling before abandonment;
  its own retained-storage assertion is the relevant boundary. No sync sleeps.
- Runner still imports the historical rootless GuestRuntime constructor; migrate
  it before M8. Broad command/network faults and all implementation adherence
  gates remain outstanding. This run is evidence, not an adherence pass.

- Follow-up drop-wait run 42448 completed exit 0. Required retained-storage
  assertion, trusted status and VM poweroff pass; evidence preserved under
  maintained-formatter-drop-wait-v1. This terminal path intentionally does not
  claim resource release. Immediate running-owner drop case now being run.

### Domain publication rollback migration and formatter abandonment result

- Immediate formatter-owner abandonment run 96375 completed exit 0; retained
  storage assertion and independent guest status/poweroff pass. Preserved under
  maintained-formatter-drop-running-v1. Together with earlier wait/drop and
  cancellation/exhaustion runs, all initially extracted formatter cases now have
  fresh native amd64 VM evidence, not full lifecycle acceptance.
- Migrated domain-publication probe into tests/lifecycle/src/domain_publication.rs
  with all four original states (absent/present/malformed/mismatched). Preserved
  original harness; no production API or policy owner added. Strengthened final
  assertion to require no child cgroup after each successful rollback, including
  absent-record case. Fixture corrupts only its own record and uses real owned
  empty cgroup domains; no commands or cross-build recovery are involved.
- New maintained --case domain-publication run passes: invalid records retain
  domain/record and reject cleanup, restored correct record permits rollback,
  all resources checked, trusted exit-zero status and guest poweroff observed.
  Strict fixture Clippy passes. Source hashes, exact migration description and
  small evidence preserved under maintained-domain-publication-v1; large assets
  explicitly await durable archival. This is test evidence, not an adherence pass.
- M4 remains 0/3. Remaining command/helper death, receiver gate failure, network,
  other partial-acquisition/writeback and broader integration cases still require
  migration and fresh runs; M5-M8 qualification/retirement gates remain intact.

### Maintained contained-helper fault coverage

- Added safe fixture exercising current RecordedDomain/ContainedHelper protocol:
  missing executable before child creation, peer exit after identity exchange
  before cgroup join, and pre-admission cancellation with a live descendant.
  No production changes, policy pins, namespace-descriptor widening or registration.
- Child pipe acknowledgment precedes helper datagram readiness. Supervisor then
  cancels the domain, reaps direct helper, drains adopted descendants and finishes
  the record; requires at least one descendant reaped and no remaining children,
  child cgroups or protected records. No synchronization sleeps. These are trusted
  owned fixture processes in the disposable VM, not escape attempts.
- maintained-contained-v1 passes all three markers, trusted exit status and guest
  poweroff. Strict fixture Clippy passes. Exact source snapshot, hashes and small
  evidence preserved; large assets explicitly await durable recovery. This is
  containment-component evidence, not mounted-sandbox/receiver/publication coverage.
- M4 stays 0/3. Receiver post-admission gate-send failure, mounted partial-setup,
  supervisor death, network and writeback fault coverage still need migration and
  current runs. No implementation adherence pass is inferred from this test.

### Abrupt contained-helper death validation

- Extended the maintained contained fixture with trusted helper abort after
  descendant readiness. No sleeps: pidfd exit observation precedes checking the
  still-populated cgroup and retained protected record. Explicit cancellation,
  direct-helper reaping, adopted-descendant drain and finish must remove all
  children, child cgroups and records; helper status must report SIGABRT.
- Fresh disconnected native amd64 VM run maintained-contained-v2 passes all four
  cases, independent status-disk exit zero and guest poweroff. Strict fixture
  Clippy passes. Small evidence and exact source snapshot preserved; large assets
  explicitly await durable archival. No production changes or policy registration.
- M4 remains 0/3. This component test does not cover mounted receiver admission,
  supervisor death, writeback, network or output publication. It is validation
  evidence, not an adherence pass. The prior goal turn was clarification only;
  this turn resumed execution and produced new lifecycle evidence.

### Namespace and filesystem acquisition rollback migration

- Migrated historical volume-supervisor-sandbox namespace/mount failure cases to
  tests/lifecycle/src/sandbox_failures.rs, using current production CommandSandbox,
  Runtime and Volume owners. No policy APIs or extra raw-descriptor exposure.
- Disposable guest only: namespace allowance is temporarily zero and restored
  before rollback; mount case supplies an unformatted dm-zero snapshot. Original
  error kind/errno survive rollback, cleanup diagnostics stay empty, actual runtime
  recovery/volume/domain directories and process children are checked, returned
  snapshot validates and explicitly closes/detaches, device disappears and runtime
  directory closes cleanly. Strict fixture Clippy passes.
- Fresh namespace-v1 and mount-v2 runs pass markers, independent exit-zero status
  and guest poweroff. Mount-v1 is preserved too; v2 strengthens the cgroup directory
  assertion to propagate enumeration/type errors instead of ignoring them. All
  exact source snapshots and small evidence preserved; large assets remain tracked
  for durable archival. No failure or historical source was discarded.
- M4 remains 0/3. Acquisition rollback does not cover mounted command execution,
  receiver post-admission gate-send failure, supervisor death, network or writeback.
  These validation runs are not adherence passes; remaining milestone scope is
  unchanged. Previous turn's committed abrupt-death regression was progress.

### Mounted preparation and abrupt attached-view helper failure migration

- Extended the maintained sandbox fixture with trusted formatting of a real ext4
  snapshot, initial scaffold creation, remount and production CommandSandbox
  preparation. Added helper launch ENOENT, exit before join, and abrupt exit after
  private idmapped root attachment. All assert live mount/record acquisition
  before injection and no children, volumes, domains or records after rollback;
  rejected snapshots explicitly validate/close/detach and runtime closes cleanly.
- Attached-exit uses Step::execute and requires SetupFailed with complete cleanup,
  never an accepted execution receipt. Helper confirms mapped root ownership and
  records attachment before exit(73), bypassing Rust destructors. No recipe runs.
- Fresh mounted-launch-v1, mounted-join-v1 and mounted-exit-v3 VM runs pass required
  markers, independent exit-zero status and poweroff. Strict fixture Clippy passes.
  An initial cutoff argument type mismatch was fixed; its compiler failure log is
  retained. No production changes or labeling policy registration introduced.
- Preserved failed mounted-exit-v1/v2 runs: fixture incorrectly placed its view
  under writable /work, and production rejected the attachment. Diagnostics in
  v2 located the issue; v3 uses existing protected runtime-parent. Production
  protections were not weakened. Every run retains exact source and small evidence;
  large run assets remain explicitly pending durable archival.
- Prior goal turn made progress with committed acquisition tests. This turn adds
  mounted-storage failure evidence, not an adherence pass. M4 remains 0/3; recipe
  admission/start-gate failure, supervisor death, network and writeback coverage
  still need completion, alongside the unchanged M5-M8 qualification gates.

### Maintained network composite ownership regression

- Migrated allocation_lock_child, allocation_lock_probe and composite_failures
  from historical network.rs without assertion changes. Added only the fixture's
  direct pinned rustix dependency, already present transitively; no production
  dependency, policy registration or raw namespace interface was introduced.
- Fresh disconnected native amd64 network-composite-v1 passes six ownership
  assertions plus independent exit-zero status and VM poweroff. Strict fixture
  Clippy passes. Evidence/source preserved; large assets await durable archival.
  The expired teardown case intentionally retains ownership for VM disposal and
  does not claim clean resource release. Actual recipe DNS/TCP remains unvalidated
  by this component test. M4 remains 0/3; no adherence pass inferred.
- Clarified the user's NAT-collision question: this fault is an nftables table-name
  collision, not an address collision. Link-local NAT is a separate design question;
  no address-policy change or link-local qualification is claimed by these tests.
  The previous goal turn's mounted rollback tests were concrete progress.

### Actual recipe DNS/TCP, resolver protection and network failure cleanup

- Added optional maintained distro-runner network fixture: safe Rust DNS/TCP
  service in the disposable VM, pipe readiness before build, finite observation
  timeout, resolver supplied through the normal CLI. Recipes use Fedora libc/bash
  to resolve build.fixture and exchange fixed request/reply; attempts to append
  to the injected resolver must fail. No production APIs or dependencies changed.
- Successful network-command-v1 build/export passes, all artifact blob digests
  verified. Following offline recipe step sees only loopback and no IPv4 routes.
  Independent trusted guest status plus service/recipe markers and post-build
  veth/nftables absence all pass. Source/context/input/artifact hashes preserved.
- network-nonzero-v1 and network-timeout-v1 each reach DNS/TCP before deliberate
  exit(7) or five-second timeout. Runners correctly fail; separate maintained
  assessments require exact expected error, cleanup complete, output absent,
  independent build rejection and all network resources removed. Both pass.
- Strict lifecycle-fixture Clippy and Python compilation pass. Small evidence,
  context bundles, verifier and source snapshots preserved; large VM assets remain
  explicitly pending durable archival. Earlier goal turn's network ownership
  migration was progress. No synchronization sleeps or policy registration added.
- Scope limitation: destination is inside the VM, so these runs do not demonstrate
  external outbound NAT or IPv6 forwarding. Nor do they close receiver start-gate,
  supervisor-death or writeback cases. M4 remains 0/3 and the broader M5-M8 gates
  remain open; these runs are validation evidence, not adherence passes.
