# Rewrite completion ledger

The standalone rewrite implements DESIGN.md and SANDBOX-DESIGN.txt using the
ownership architecture in ARCHITECTURE.md. The user deferred confidential
computing qualification; native Arm64 CI remains part of acceptance.

| Milestone | Success criteria and evidence | Adherence |
| --- | --- | --- |
| M1 Preservation | Original code, uncommitted work and validation provenance recoverable; ARCHIVE.md and archival receipts | 3/3 |
| M2 Architecture | Linear pipeline, owned resources, sealed state traits, immutable validated inputs and consuming transitions; architecture and revision reviews | 3/3 |
| M3 Foundation | Safe Rust, explicit relative recipe/lock inputs, captured committed bytes, verified local locked cache and schemas; context revision reviews | 3/3 |
| M4 Lifecycle | Restricted command namespaces, external network/label policy, mapped kernel mounts, bounded execution/cleanup, failure ownership and VM disposal; linear lifecycle and Nix compatibility reviews | 3/3 |
| M5 Bootstrap | Trusted ext4 formatting directly on dm-zero snapshot, isolated tool/output roots, both-root labels and direct scute sealing; linear bootstrap review | 3/3 |
| M6 Pipeline | One scute per carapace stage, confined COPY, firmware pair, transforms, deterministic inventory and complete atomic OCI publication; linear pipeline review | 3/3 |
| M7 Qualification | Representative Fedora/Debian/Alpine/NixOS builds and boots, arma firmware, resolved review findings, native amd64/Arm64 CI; m7-final review and QUALIFICATION.md | 3/3 |
| M8 Retirement | Verified durable recovery, maintained regression fixtures, removal of legacy/experimental trees, coherent current docs; m8-final review | 3/3; final CI required |

Three consecutive clean self-review passes are required for each milestone.
These inspect requirements, Rust style, state/ownership, error paths and the
limits of evidence; repeated test runs do not count as passes. Findings reset
the relevant streak. The retained reviews live in evidence/reviews; complete
historical ledgers, failed attempts and raw Claude discussions are archived.
No independent external approval is implied by self-review counts.

Production Rust was last changed at 5d1f358. Subsequent changes qualify real
workloads, maintain tests/CI and retire historical material. Native Arm64 and
amd64 Rust and seven lifecycle cases passed in runs 40052820789 and 40052820734.
Final branch CI also exercises the migrated ORAS graph-copy assertion in an
amd64 disposable guest. The exact final commit/run receipts are retained under
/storage/conglobate-validation/final-preservation/final-ci/ after completion;
this milestone must not be declared complete unless both workflows succeed.

The accepted environment provides Git objects, ID ranges, network namespace/DNS,
formatter runtime, fixed label exception and independent VM destruction. The
binary does not implement those environment services. Confidential hardware,
attestation, additional filesystem backends and universal distro/architecture
coverage are explicitly outside this completion claim. QUALIFICATION.md states
actual tested versions and limitations.

Retired reference/, validation/, policy/ and preservation/ trees and superseded
research ledgers are absent from the active build. Their contents, including
user review streams, remain in verified archives. Current tests/environment
contains only the trusted VM fixture policy, not a binary runtime dependency.
