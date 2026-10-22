# Conglobate reimplementation goal

Status: active. No milestone may advance until its three-pass gate is clean.
The target is the latest DESIGN.md, SANDBOX-DESIGN.txt and user decisions.
This ledger describes required work, not claims about the current executor.

## Milestones and acceptance criteria

| Milestone | Deliverables and success criteria | Status | Clean streak |
| --- | --- | --- | --- |
| 1. Preserve and separate | Preserve full working-tree implementation, uncommitted changes, tests and all validation sources/evidence; verify archive hashes; record Git baseline and external-artifact dependencies; keep recoverable originals before moving old code outside the new active build. | Complete | 3 |
| 2. Architecture contract | Define owning modules/types, sealed state traits, consuming transitions, fallible finalization and recovery, confined filesystem authority, typed formatter properties, inventory migration, and requirements-to-tests traceability. Identify external qualification requirements without claiming access. | In progress | 0 |
| 3. Inputs and planning | New safe Rust crate; required context-relative recipe/lock paths; exact committed context and local verified image references; one recipe mapping, immutable plan, complete stage validation/inheritance; meaningful positive/negative tests. | Pending | 0 |
| 4. Privileged lifecycle | RAM workspace, cache-backed verified layers, kernel/idmapped mounts, scoped LSM, no command SYS_ADMIN in any capability set; private namespaces, restricted descriptors/syscalls, bounded execution/setup/teardown, cancellation and descendant quiescence; partial-setup/recovery tests and no context/control/block-device exposure. | Pending | 0 |
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
