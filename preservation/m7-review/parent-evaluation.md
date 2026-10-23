# Parent evaluation — provisional, not consensus

Claude's initial review is still running. These are independent checks of early
agent claims; the final critic dispositions and follow-up response remain required.
The baseline remained unchanged during the initial parent checks. A subsequent
stable-toolchain check found one redundant inventory-validation expression;
the equivalent simplification is recorded below as a post-baseline change.

## Mechanical evidence

`parent-checks.json` records successful direct execution of 197 rootless tests,
54 doctests and strict all-target Clippy on fd37cc7. Claude's own mechanical
commands were denied; these parent results must not be attributed to Claude.
The earlier `fmt.log` records a successful format check. Privileged VM evidence
remains separate; no privileged tests were run on the host.

The parent subsequently ran instrumented rootless tests with cargo-llvm-cov 0.8.4
(197 passed) and preserved full and per-file summary reports. The tool warned
that its initial package clean failed due to a missing CACHEDIR.TAG; compilation,
tests and report generation proceeded successfully. This warning remains in
`rootless-coverage.log`. Env-cleared child processes wrote four default LLVM
profiles in the repository cwd; `profile-fragments.json` records their hashes.
They were preserved and merged into the final report without rerunning tests.
The report includes test code and no branch instrumentation or VM coverage;
its aggregate line percentage must not be presented as overall qualification.

## Early dependency findings

- dependencies-001: the missing advisory run was a coverage gap, not evidence
  of a high-severity vulnerability. Parent `cargo audit --json` completed with
  exit 0, database commit f7b2f6bc663452200bbe639ebb75e0c762b3422b, updated
  2026-10-23, reporting zero matching vulnerabilities and no warnings across
  246 lockfile dependencies. See `audit-before.json`. This does not certify
  dependencies as vulnerability-free. Missing automated CI remains open in M7.
- dependencies-002: source-specific advisory coverage needs qualification.
  Cached rustsec 0.33.0 `src/database/query.rs:181-194` compares advisory source
  kind and URL against the package. Git sources are not categorically impossible
  to match, but ordinary crates.io advisories do not match a git source. Keep
  that actual limitation explicit; do not claim the audit certifies pinned git
  revisions.
- dependencies-003: reject the claim that the rustix pin has no documented
  purpose. `ARCHITECTURE.md:247-257` documents the unreleased safe mount_setattr
  API and its VM qualification. `src/filesystem.rs:129-135` uses borrowed
  MountAttrParamFlags::IdMap and mount_setattr. A Cargo.toml pointer would improve
  discoverability; replacing the pin merely because version strings coincide
  would discard a required API.
- dependencies-004/011: duplicate packages exist, but the lockfile does not prove
  that an entire syscall or hashing implementation is duplicated in the final
  binary, nor that a current correctness/security failure follows. A global
  crates.io patch would also change transitive dependencies and needs a reason.
- dependencies-005/006: configuration failures reproduced. An unrestricted
  offline deny check first failed because foreign-platform crates were uncached
  (`deny-before.log`), so it is not a completed license/source check. The host
  target check then ran and exited 12 (`deny-host-before.log`): rustix source
  not allowed; BSD-2-Clause not allowed for libbpf-cargo, libbpf-rs and libbpf-sys;
  BlueOak-1.0.0 not allowed for minicbor. The latter was missed by the review.
  libbpf-sys itself declares BSD-2-Clause; do not describe this as demonstrated
  accidental LGPL selection. Configuration needs correction and rerun.
  A separate proposed configuration (`proposed-deny.toml`) adds BSD-2-Clause,
  BlueOak-1.0.0 and only the specific rustix Git URL. Cached license texts and
  package declarations were inspected. Separate offline amd64 and arm64
  license/source checks both exit 0 (`deny-proposed-amd64.log` and
  `deny-proposed-arm64.log`). A combined multi-target invocation could not
  resolve uncached foreign-platform crates; that failed attempt is also retained.
  This proposed configuration is not yet applied to the repository's deny.toml.
- dependencies-007/008: explicit toolchain/build prerequisite documentation is
  useful and remains part of M7 repository usability. Do not invent an MSRV of
  1.85 from edition alone: dependencies and used language features determine the
  actual floor. build.rs currently relies on SkeletonBuilder diagnostics.
- dependencies-009: exact pins are not themselves a demonstrated bug. This
  sandbox deliberately qualifies concrete implementations. An update procedure
  and advisory checks are warranted; automatic relaxation is not justified.
- dependencies-010: reject the claimed compiled network client surface based
  solely on the package name. `gix-transport-features.log` shows only its empty
  default feature. Cached gix-transport 0.57.2 `src/client/mod.rs` gates async_io,
  blocking_io and git behind async-client/blocking-client, neither enabled.
  Replacing gix's repository/object handling needs a demonstrated benefit.
- dependencies-012: unexamined default features are a review task, not an issue
  without naming an unnecessary enabled feature and its concrete effect.
- dependencies-013: active CI is indeed absent, already an explicit outstanding
  M7 criterion. Consolidate with missing advisory automation rather than count
  the same missing gate twice.

All dispositions above are provisional until Claude evaluates the evidence.

## Early ownership and test findings

- ownership-001/002 identify avoidable path and scratch-buffer allocations.
  They do not demonstrate a high-severity performance regression: no measured
  workload or elapsed-time effect was supplied. The small local improvements
  are plausible, subject to the existing COPY behavior tests.
- ownership-009's suggested changed flag is not equivalent to current behavior.
  Multiple edits can change a value and then restore it. Comparing the final
  tree with the original preserves the exact original DTB bytes in that case,
  including layout/padding not necessarily reproduced by an encoder. Any
  optimization must preserve this invariant; merely reporting that one op
  changed a property would not.
- tests-001/002/003/005/009 correctly distinguish rootless tests from privileged
  integration, but absence of in-file tests is not absence of evidence. The
  retained VM harness exercises seccomp/capability denials, adoption and drain,
  chain verification, setup/teardown failures and real CLI output gates. The
  missing CI rerun remains a real M7 requirement. Golden hashes and duplicated
  allowlists alone would mostly mirror implementation; prefer behavioral
  assertions of the security boundary. Review exact uncovered branches before
  adding seams to production ownership merely for test access.
- tests-004: an aarch64 parser branch test is a reasonable coverage addition.
  The broader allegation of overstated hardware coverage conflicts with explicit
  outstanding qualification in M7-QUALIFICATION.md and the milestone ledgers.
- tests-008: reject the assertion that YAML has no configured depth/size limits.
  `YamlDocument::decode` preserves `Options::default().budget`. Cached
  serde-saphyr 2.0.0 `src/de/options.rs:613` sets Some(Budget::default());
  `src/de/budget.rs:275-299` includes depth 64, event/node/scalar/anchor limits
  and alias ratio enforcement. Additional local boundary tests may be useful,
  but must not describe default protections as absent.
- tests-010: the cited negative timeout test first completes peer registration;
  its peer remains alive until explicitly terminated after the timeout assertion.
  Scheduling delay does not turn that peer into an expected early exit. A larger
  timeout does not establish the proposed improvement and increases runtime.
- tests-011: these tests intentionally exercise RAM-backed materialization;
  silently falling back to ordinary TMPDIR would violate that prerequisite.
  Document the writable tmpfs requirement rather than skip required coverage.

These are provisional assessments, not a substitute for Claude's critics or
the requested source-grounded follow-up exchange.

## Early performance findings

- performance-001: the separate reference and chain checks do duplicate digest
  work on scutes. However, the later preparation read is not interchangeable
  with the earlier check: it holds the exact inode backing the mounted device
  and verifies its root against the trusted root. Cache contents can change
  between reads. Any consolidation must keep per-read authentication and input
  receipts; do not remove mount-time preparation as redundant.
- performance-002: data_blocks describes rounded block geometry, not the exact
  decoded byte length. ScuteExtent explicitly requires the known length and
  preserves canonical padding and late-decoder-error checks. The suggested
  unknown-length-at-EOF rewrite is not a drop-in equivalent and conflicts with
  the user's explicit fixed-length verity requirement. Cost is real, but no
  measured bottleneck or safe equivalent implementation was demonstrated.
- performance-003: inspect the pinned devmap formatter before asserting syscall
  counts; the reviewer explicitly did not inspect its internal buffering.
  Parent inspection of devmap 2679426 `verity/src/format/sync.rs:103-120`
  finds io::copy for data, rather than an explicit one-read-per-4096-block loop.
  `format/tree/sync.rs:23-42` seeks before every pending tree block write;
  a standard BufWriter would flush at those seeks, so the suggested output
  buffering does not automatically batch those writes. Measure before claiming
  the proposed fix removes this cost.
- performance-004/005/006/009: plausible cost reductions, but directory-handle
  reuse must preserve confined symlink resolution and COPY semantics. These are
  unmeasured optimization opportunities, not demonstrated security defects.
- performance-007: decoded unannotated input has no absolute application size
  ceiling. DESIGN.md:48 and :237-238 explicitly assign resource limits to the
  environment and permit resource exhaustion to fail a build. A 64 GiB logical
  root-volume limit is not automatically a valid upper bound on a snapshot COW
  file including metadata. Review a wire-format-derived bound separately from
  proposing a new resource policy; do not invent a limit solely to close this
  finding.
- performance-008: the claim that Rust stderr is line-buffered like stdout is
  incorrect. Also, holding build logs until a large buffer fills or a command
  ends changes diagnostic latency. Need actual syscall/workload evidence before
  choosing that tradeoff.
  Installed toolchain source confirms std/io/stdio.rs stores LineWriter for
  Stdout but StderrRaw directly for Stderr. alloc/io/buffered/linewritershim.rs
  uses the last newline in a supplied chunk, so even line-buffered stdout does
  not imply one write syscall per newline in that chunk.

## Early concurrency findings

- concurrency-001: do not implement an unconditional relay join after a failed
  wait. `DiagnosticRelay` explicitly permits join only after namespace init and
  descendants have exited. In `receiver.rs:135-148`, the successful wait branch
  drains diagnostics; the failed wait branch retains the running mount and
  returns an error through the exit-only helper. A pipe writer may still be
  alive, so the suggested join can block. The supervisor cancels the domain
  under its deadline. Losing final logs on timeout is different from losing
  ordinary failure diagnostics. Check the retained output-timeout VM case before
  treating this as an unbounded detached-thread bug.
- concurrency-002: reject accepting teardown success merely because the last
  child may have exited at an already-expired deadline. The final quiescence
  observation has not yet occurred, and the design requires completion within
  the original budget. A strict boundary rejection is not demonstrated flakiness.
- concurrency-003: inspect actual callers. These deadline-free wrappers cannot
  consume or silently renew a supplied SetupCutoff because they take none;
  production helper dispatch passes the cutoff to deadline-aware execution.
  Specifically, `isolation/setup/gated/helper.rs:35-40` calls
  `self.enter_until(cutoff.deadline()?)` and rechecks before spawning; the root
  transition checks the same cutoff before and after restriction and at its
  start gate. Current deadline-free namespace callers are validation fixtures.
  Unused convenience wrappers may be removed as API cleanup, but a production
  budget-renewal bug requires a reachable caller that discards its original
  cutoff.

## Early unsafe/FFI findings

- unsafe-ffi-001: the proposed empty-argv trigger needs to account for Linux
  exec behavior. Upstream fs/exec.c adds an empty argv[0] when argc is zero and
  sets argc to one. This is present even in the older
  [v6.6.37 source](https://kernel.googlesource.com/pub/scm/linux/kernel/git/stable/linux-stable.git/+/refs/tags/v6.6.37/fs/exec.c),
  preceding the required Linux 7 environment. A defensive slice accessor is
  harmless, but the reported current-kernel panic trigger is not established.
- unsafe-ffi-002: inspect the actual pinned devtree source instead of treating
  it as unreadable. Revision e90bafc `src/header.rs:26-28` rejects input shorter
  than FDT_HEADER_SIZE (40); `src/owned.rs:673-693` reserves that header and its
  writer emits the fixed 40-byte header. The 24..32 ranges are within the
  dependency's verified representation. No concrete short-buffer path supplied.
- unsafe-ffi-003: admission and quiescence ordering is precisely why these map
  writes do not require concurrent-reader atomicity for untrusted subjects.
  Step::admit_until activates before opening admission, and Step<Finishing>
  can disable only after drain and finished domain recording. A comment at the
  raw map update sites would improve local documentation; changing the map
  protocol without a reachable concurrent subject is not justified by this
  explicitly hypothetical finding.

## Early API-surface findings

- api-surface-002: the suggested pub(crate) associated resource payload does not
  compile while the sealed public trait and implementation remain public.
  `associated-visibility-rejected.rs` reproduces E0446 (rustc exit 1); the same
  shape with a pub type in a private module and a public associated-type
  projection compiles (exit 0, `associated-visibility-valid.rs`). This is a
  necessary Rust visibility distinction, not a private-interface leak. The
  projection also gives consumers a way to name the associated type.
- api-surface-003/004/005: the reviewer explicitly excluded validation/ as
  non-current. That contradicts the supplied scope, which includes the active
  validation/rewrite-command-integration sources as supporting evidence. For
  example main.rs:939 imports ExecutionOutcome, :761 uses CommandPreparation,
  :2373 constructs Step; formatter.rs:221 uses Inventory; sealed.rs:24 uses
  AssembledImage. Numerous other named alleged orphans are live there. Also,
  return/error types can be used through inference without explicit type-name
  references. Recompute the caller analysis before reducing visibility.
- api-surface-006: directly false. `src/output/assembly.rs:437-439` uses
  `crate::firmware::tests::Fixture::image` and `Fixture::detached`, so narrowing
  the fixture module to private would break existing sibling tests.
- api-surface-001: an unexported concrete error payload can be consumed through
  inference and its methods. An extra re-export is an ergonomics decision,
  not proof of a broken recovery path. Evaluate actual external callers before
  choosing between narrower API and an added export.

The clean-result claim of no proc-macro dependency in unsafe-ffi is also false:
serde derives, thiserror and clap derive are in use. Application unsafe remains
forbidden, but do not use that mistaken premise in the final safety summary.

## Early error/API findings

- errors-api-001 duplicates unsafe-ffi-001; the same Linux argv normalization
  counter-evidence applies.
- errors-api-002/003/008 identify loss of typed error sources at diagnostic
  boundaries. The CLI currently reports failure uniformly and the exit taxonomy
  is explicitly deferred in DESIGN.md. This is an API/diagnostic improvement,
  not a demonstrated misclassification of successful output. Borrowed cleanup
  owners must not be boxed as if they were 'static, nor dropped early merely to
  retain a source chain. A concrete owned diagnostic representation needs to
  preserve the existing failure ownership and both primary/cleanup messages.
- errors-api-005 is a useful, concrete conversion improvement: validate decoded
  u64 capacity through TryFrom<u64> and share it with FromStr, instead of
  formatting bytes back into human input syntax. Preserve all current bounds.
- errors-api-006 is a useful diagnostic improvement: Display should report the
  primary outcome and cleanup diagnostics, not Debug-dump all retained owners.
  `cleanup_error()` already exposes the relevant summary. Recovery ownership
  should remain unchanged.
- errors-api-007: there is already checked_add at execution admission in
  step/setup.rs:106. Making parsing clock-independent is reasonable, but the
  suggestion to introduce a new fixed policy maximum is not part of the agreed
  finite-timeout contract. Keep representational overflow checks and the actual
  admission check; do not invent an arbitrary duration limit.
- errors-api-009/010 are small clarity changes, not demonstrated reachable
  panics: the early Complete return establishes the later unreachable arm, and
  minicbor Vec writing is infallible. A single match and explicit expect reason
  can express those existing invariants more clearly.


## Early security findings

- security-001/002 duplicate performance-007's resource-policy question.
  The environment owns resource limits under the agreed design. Requiring an
  optional wire annotation or treating the logical root volume size as a bound
  on a snapshot COW extent would change supported input semantics. A concrete
  format-derived consistency bound is worth considering separately; these
  suggestions do not establish one.
- security-003 is a useful hardening candidate requiring precise runtime checks.
  ProcRestrictions currently makes sys/sysrq-trigger/irq/bus read-only; it does
  not mask the additional listed entries. A private PID namespace restricts
  process visibility, but not every global proc entry. Kernel configuration,
  DAC and kernel pointer restrictions affect which entries disclose what, so
  "readable" is not established merely by an omitted overmount. Evaluate empty
  file/directory masks against the actual namespace and qualified kernel, then
  test positive ordinary /proc use and negative global disclosure in a VM.
- security-004 identifies an allowlist policy choice, not a demonstrated current
  vulnerability. The design trusts the kernel and requires representative distro
  workloads. Historical vulnerabilities alone do not establish that the current
  syscalls allow a breakout, and pipe transfer operations are not intrinsically
  outside OS build workloads. Ask for concrete compatibility evidence before
  removing them; do not silently narrow the intended build environment to the
  synthetic fixture's syscall set.
- security-005 correctly observes that the formatter is privileged. That is an
  explicit trust boundary in SANDBOX-DESIGN.txt 1.2 and 5.1, not a command-sandbox
  escape: recipes cannot supply arbitrary formatter code or arguments. Further
  least-privilege confinement can be useful defense in depth, but needs a
  separate formatter capability/syscall profile and qualification; applying the
  command profile blindly is not justified. No untrusted-command path into the
  formatter role was demonstrated.
- security-006 conflates a low-level filesystem operation with the assembled
  artifact contract. StagedDirectory::publish explicitly documents that it is
  not a build-success receipt, and the production coordinator verifies through
  AssembledImage. A trusted Rust caller can already write/rename arbitrary host
  directories; library visibility is not the command security boundary. API
  narrowing can still be considered, checking current integration callers.
- security-007: the entry point clears ambient and inheritable capabilities,
  while capset and selected prctl operations let commands manipulate their
  already-bounded credentials. The design requires SYS_ADMIN absent from all
  sets, not all ambient capabilities permanently empty. Any raised ambient
  capabilities remain bounded by the allowed set. Query-only PR_CAP_AMBIENT
  filtering could be a policy refinement, but the reported violation of a
  persistent ambient-empty design invariant is not established.
- security-008 repeats the already checked devtree length invariants in
  unsafe-ffi-002. Checked slicing would make the boundary defensive locally;
  current source does not demonstrate a malformed-input panic at that slice.

The security review did not itself inspect the complete current VM harness.
Its "not checked" entries must remain scoped to that reviewer, rather than
asserting that the project has no BPF, seccomp, network or teardown evidence.


## Stable-toolchain checks and one post-baseline correction

`stable-checks.json` records the additional installed Rust/Cargo 1.94.0 checks.
All 197 rootless tests and 54 doctests passed. Strict Clippy initially rejected
an inventory-validation expression as nonminimal; the original failure log is
retained. Removing the redundant authored-and-absent-target term is logically
equivalent: skipped authored transforms fail the first term, while every
non-skipped absent target fails the second. Strict Clippy and all 197 tests pass
after the simplification; the format check also passes. This is the only
production source change since the independent review's baseline.

A rootless GitHub workflow and build/prerequisite README are now authored.
The checkout action is pinned to the v7 commit resolved from the upstream Git
ref (3d3c42e5aac5ba805825da76410c181273ba90b1), with credentials not persisted.
The workflow has not run on GitHub. Dependency-policy automation, portable VM
execution, distro and platform qualification remain open; no clean M7 adherence
pass is claimed by these checks.


## Correctness finding

- correctness-001 is confirmed. CapturedFormatter stores ContextPath keys in a
  BTreeMap; FormattedSnapshot::into_recorded_parts preserves that order, as does
  Formatter::try_from. Formatter::validate instead compares serialized String
  paths. The standalone safe Rust reproduction in path-order.rs/log demonstrates
  the disagreement for usr/lib/a and usr/lib-x. This can reject a valid trusted
  formatter runtime at inventory construction, so it is a correctness bug,
  rather than merely a low-severity style concern. Proposed correction: sort
  the serialized runtime records by their string path during receipt conversion,
  preserving the existing inventory canonical ordering. A VM regression should
  include both names in a real captured formatter runtime and require successful
  inventory emission/round-trip; do not add a forgeable production receipt
  constructor solely to test this conversion.


## Documentation findings

- documentation-001/002 are confirmed stale comments: CARAPACE injection and
  the pipeline/publication paths are implemented. Correct the text; the stale
  sentence alone is not a high-severity execution/security bug.
- documentation-003 is useful. ARCHITECTURE.md's early conceptual state names
  should be reconciled with the actual consuming methods and ownership types.
  Document normal execution and failure/recovery transitions, without implying
  that a single linear path covers all cancellation or retry outcomes.
- documentation-004/007 identify useful public ownership documentation additions.
  Distinguish destructor fallback from explicit fallible cleanup and from a
  quiescence/durability receipt. The private commit method's no-restoration
  comment is true for that method, not a contradiction of default guard Drop.
- documentation-005: primary operation summaries and error/recovery obligations
  are useful additions. Focus on behavior callers must uphold, not a boilerplate
  repetition of each error enum or invented panics.
- documentation-006 overstates the problem. Dropping ChildReaper naturally leaves
  subreaping enabled precisely because it has no custom destructor. The existing
  sentence states the observable behavior correctly; clarification is optional,
  not a missing destructor bug.

The claim that pinned devtree source was unavailable is contradicted by the
parent's cached-source inspection recorded above. tests/ and the current
validation/rewrite-command-integration sources were explicitly supporting scope;
reviewers excluding them must not turn that omission into claims of absent
coverage or dead public APIs.


## Initial critic scope remains incomplete

The Claude coordinator reports 111 raw findings and selected about 70 medium
or higher/substantive-low findings for 14 critic batches, leaving about 41 low
findings uncritiqued. This is not a disposition for the omitted findings. The
follow-up must finish their source-grounded evaluation as well as negotiate the
counterexamples above. No full-review consensus is claimed from that subset.


## Correctness-001 regression and fix

v259 reproduced the actual failure in the disposable VM: the trusted formatter
and all three network-enabled pipeline commands completed, then conglobate
rejected the runtime ordering during inventory construction. The new fixture
includes usr/lib/conglobate-order/a and usr/lib/conglobate-order-x as real
captured files. The original failure, source archive and logs are retained.

Formatter::try_from now sorts RuntimeFile values by the serialized String path.
v260 passes the entire existing VM suite with those same files, including the
real formatter receipt's inventory round-trip and explicit string-order check,
CLI publication and ORAS copies. Source archives are captured before building
from now on. Rootless tests (197), strict production and fixture Clippy, and both
format checks pass; ordering-checks.json records the evidence. This confirms
and fixes the bug independently; Claude's final disposition/consensus is still
pending. No distro or confidential-platform qualification is inferred.


## Lifecycle documentation and explicit capability contract

The stale crate/CARAPACE comments and conceptual architecture state names are
corrected. Step, policy and supervision docs describe actual transitions and
failure retention; Volume and namespace docs explain destructor fallback. The
primary APIs now describe error and caller obligations. ChildReaper wording is
clarified without adding a destructor. Strict rustdoc and all 54 doctests pass.

The syscall/capability test findings need calibrated dispositions: current VM
fixture main.rs:2153-2205 applies the actual policies and checks thread creation,
ordinary I/O, denied user namespace creation, all five capability sets and
no_new_privs. This is real execution coverage, not zero testing. Its allowed-set
expectation is derived from the policy, however, so a new independent rootless
contract test now asserts that filesystem commands never receive SYS_ADMIN.
This tests the explicit design prohibition, not a duplicated allowlist or BPF
hash. The rootless suite is now 198 tests; strict Clippy and formatting pass.

Critic verdicts marked rejected merely because a finding was fixed during the
review must be recorded as fixed, not counted as original false positives.
This applies to correctness-001 and the remediated documentation findings.
The original review baseline and all failure/regression evidence remain retained.
