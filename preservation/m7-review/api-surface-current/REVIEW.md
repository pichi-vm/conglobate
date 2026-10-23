# API surface recount and current-source dispositions

At d9ecd56, no production change. Scope and exact lexical hits are in recount.json;
recount.py reproduces the inventory. It covers 199 Rust files in src, tests and
validation harness src directories, including doc comments. 184 candidate names
from the two original clustered suggestions: 37 have external spelling; 14 have
no spelling in this scope. These are name counts, not semantically resolved types.
Two stopped initial scans included old evidence snapshots (first all, then
reproduced copies); their scripts are preserved. Only the final inventory is used.

- api-surface-001: current UnmountFailure is a public struct with private fields;
  UnmountRecovery is private and reachable only through consuming cleanup. The
  originally cited public enum payload no longer exists. Reject the stale claim.
- api-surface-002: MountedResources is the associated Resources of public sealed
  VolumeState. Its fields are private; public type visibility does not expose
  its owning handles. Narrowing the associated payload triggers the previously
  reproduced E0446 constraint. Keep that rejection; no fresh compiler experiment
  or claim of new execution evidence is made.
- api-surface-003: maintained tests/lifecycle directly uses step, network,
  isolation, filesystem and formatter. Historical harness source also names
  output and inventory; those references need migration, not erasure. Public
  BuildError payloads name inventory/firmware/transform errors, so a source scan
  limited to spelled caller names cannot establish the proposed module demotion.
  Reaffirm rejection of the blanket nine-module change.
- api-surface-004: ExecutionOutcome is matched in maintained sandbox_failures;
  RuntimePaths and multiple failure owners are inferred from public return values.
  CommandDomain::create -> DomainCreateFailure and DomainTarget::remove ->
  DomainTargetFailure are public signature dependencies even when callers use ?
  or inferred matches. Fourteen named candidates are already absent after prior
  refactors. Keep rejection of the asserted orphan cluster, not a claim that
  every remaining exported name is minimal.
- api-surface-005: completed the requested properly scoped recount plus signature
  inspection. CommandSyscalls and ChildReaper have direct maintained consumers;
  VolumeState/PreparationStorage/StepState carry public associated types and
  immutable state transitions. Public Result types, inferred error owners and
  associated payloads invalidate bulk demotion from grep counts. Do not implement
  the blanket list. Candidate-specific future simplifications still need an
  actual call/signature analysis and a demonstrated reduction in complexity.
- api-surface-006: output/assembly.rs calls firmware::tests::Fixture::image and
  ::detached. The sibling test consumer requires crate visibility; retain rejection.

This does not assert complete semantic reachability of all 184 names or certify
historical harnesses compile against current API. The broad claims are refuted by
current concrete consumers and signature constraints. No production API changed,
so no redundant Cargo/VM test run. Existing 225 all-target/53-doctest checkpoint
covers unchanged production source. Full M4 review, migration of historical tests,
UID/GID contract, platform qualification and final API decisions remain open.
