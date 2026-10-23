# Parent evaluation — provisional, not consensus

Claude's initial review is still running. These are independent checks of early
agent claims; the final critic dispositions and follow-up response remain required.
No production fixes have been applied during this baseline review.

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
