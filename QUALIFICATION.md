# Qualification

The standalone rewrite has completed the M1–M7 implementation and qualification
gates. Final preservation/retirement is recorded in REIMPLEMENTATION.md and
ARCHIVE.md. This is evidence for the stated cases, not proof that all hostile
workloads, kernels, distributions or deployment environments are safe.

| Coverage | Result and scope |
| --- | --- |
| Fedora | Real DNF transaction, target-policy labels, enforcing SELinux boot and label/ownership checks; native amd64, Fedora v3 evidence |
| Debian | APT install, systemd boot, package database and ownership checks; native amd64, Debian v2 evidence |
| Alpine | New bootstrap root, APK/musl and OpenRC boot; native amd64, Alpine v2 evidence |
| NixOS | Pinned Nix/nixpkgs, real system closure build/export, layered ownership marker and systemd activation/boot; native amd64, NixOS v6 evidence |
| Firmware generation | New initramfs and arma-generated PMI/DTB, final CARAPACE binding and successful boot; supplied Fedora kernel, firmware v1 evidence |
| Full pipeline | Direct dm-zero bootstrap, multiple carapace directives producing one stage scute, alternate firmware tools, final DTB binding, deterministic inventory and exact OCI blob graph |
| Failure handling | Admission rejection, capability/namespace restrictions, cancellation, blocked I/O, helper/supervisor death, retained cleanup authority and output exclusion; detailed cases in the archived lifecycle evidence |
| Native architectures | Rust formatting, Clippy, rootless tests and doctests pass on amd64 and Arm64 |
| Guest lifecycle CI | Cache integrity, containment, runtime scope, private/changed namespace handling, clone restriction and PTY compatibility pass on amd64/KVM and Arm64/TCG |
| Confidential computing | Deferred by user decision; no Intel TDX, AMD SEV-SNP or Arm CCA hardware qualification claimed |

The pre-simplification production source was 5d1f358. Its hosted rootless/dependency checks are
[run 40052820789](https://github.com/pichi-vm/conglobate/actions/runs/40052820789);
seven lifecycle cases on each architecture are
[run 40052820734](https://github.com/pichi-vm/conglobate/actions/runs/40052820734).
Later retirement changes updated documentation and test packaging. The binary
simplification is recorded separately below; prior results retain their original
source identities.

Arm64 Rust checks execute natively on ubuntu-24.04-arm. Its guest cases use a real
Arm64 Linux kernel under explicitly recorded TCG because hosted KVM is absent.
This validates functional architecture paths, not hardware acceleration or a
production TCG sandbox. Distro boots above are amd64; the complete
four-distro/two-architecture cross-product is not claimed.

Fedora and Debian results predate the final lifecycle simplification and remain
evidence at their recorded source snapshots. Current-source integrated lifecycle,
labeling, networking, pipeline, Alpine/NixOS and firmware evidence covers the
subsequent implementation; those older distro runs are not relabeled as fresh.
NixOS and the firmware case reuse/build from supplied kernels; kernel compilation
and appliance transport/attestation are separate work. Boot harnesses stop after
the success marker and do not qualify guest-initiated shutdown.

The original review's 111 findings have source-grounded dispositions. The bounded
Claude follow-up resolved stale network/thread-count concerns; an interrupted
broad review is not represented as independent approval. Required implementation
findings are closed. Optional additional trusted-formatter hardening and syscall
narrowing are not established isolation defects under the agreed trusted-kernel
boundary. Three consecutive source/evidence self-reviews close each milestone;
repeated test runs are not review passes.

Selected reports, inventories, CI records and boot logs are under `evidence/`.
Their historical paths refer to the full verified worktree/external archives in
ARCHIVE.md. Failures are preserved alongside successes, including Nix PTY,
parent-death-signal, nested-filter, unsigned-local-copy and Arm EFI packaging
failures. Maintained recipes and regression fixtures are under `tests/`.

## Binary simplification

Production source 56c04b9 removes the public library, reusable failure-storage
APIs and redundant lifecycle/storage/network wrappers. See SIMPLIFICATION.md
for the complete review ledger and evidence/simplification for selected receipts.
Rootless validation has 213 passing tests and Clippy with warnings denied.
Focused disposable-VM runs cover failed close with a live reader, runtime cleanup,
formatter timeout/exhaustion, launch rejection, held/missing cleanup acknowledgment,
busy finalization, writeback failure and fixed teardown expiry. The full pipeline
and DNS/resolver/offline-step artifact verifiers pass. These are new build/lifecycle
results, not new distro boots or confidential-computing qualification.

The binary no longer runs library doctests. Former compiler-negative examples are
retired explicitly; fixture compilation does not replace their assertions.
The 17 removed rootless tests constructed the retired error/retry representations;
actual failure-path fixtures now assert terminal rejection and retained resources.

Final hosted acceptance requires both workflows for the pushed review-branch head:
[Rootless checks](https://github.com/pichi-vm/conglobate/actions/workflows/checks.yml?query=branch%3Arewrite-qualification)
and [Disposable VM lifecycle](https://github.com/pichi-vm/conglobate/actions/workflows/lifecycle.yml?query=branch%3Arewrite-qualification).
These include native amd64/Arm64 checks and amd64 guest ORAS interoperability.
The hosted records identify the exact commit and conclusion; local observations
alone do not satisfy this gate. Documentation-only descendants do not change the
production source identity above.
