# Milestone 5 adherence review

Status: second review found R2 documentation divergence; clean streak **0/3**.
M5 is not accepted. The earlier 1/3 pass and scope remain historical.
Starting implementation: `1cd3b69`. This review covers changes since the M4
accepted implementation scope `ed0fecd408562aed8bc083e8fbe412c4b80ec514`.
The complete milestone boundary remains DESIGN.md, SANDBOX-DESIGN.txt,
ARCHITECTURE.md and REIMPLEMENTATION.md; evidence does not narrow that boundary.

## R1 — unfinished formatter inputs dropped without a reaping proof

`Formatting` held `Process<Running>` followed by an ordinary boxed `FormatInputs`.
`FormatWaitFailure` owns that same `Formatting`. Dropping either first invoked
`ChildLease::drop`, which explicitly performs only bounded best-effort termination
and may return without reaping. Normal field destruction then removed formatter
runtime backing and requested snapshot teardown. Kernel deferred removal protects
live device holders, but it is not the explicit quiescence proof required by the
owner's documented lifetime guarantee. The runtime and mapping owner must remain
retained if explicit reaping has not completed.

Correction: retain the input box with safe `ManuallyDrop`, as active command
storage already does. Release it only after successful `wait_until` or successful
`terminate_until`. Launch failure has no child and retains ordinary returnable
inputs; completion failure follows a successful reap and may return inputs.
Dropping a running formatter or a wait/cancel failure retains inputs for dedicated
VM disposal. No watchdog, synchronization sleep, unsafe code or new dependency.

Validation:

- v183: fixed-deadline timeout/cancel/reap and real mke2fs snapshot-exhaustion
  rejection still pass, including explicit runtime/backing cleanup.
- v184: drop the running formatter owner; its snapshot mapping remains present.
- v185: reach the actual setup deadline, then drop the wait failure; its snapshot
  mapping remains present. Both drop cases are terminal disposable-VM tests and
  do not attempt another build or recover the retained owners.
- v186: full normal VM suite passes, including successful formatting, bootstrap
  execution acceptance, metadata persistence and verified sealed output reopening.
  R1 is addressed; this does not complete the full milestone review.
- 56 library tests, 49 doctests, root and fixture strict Clippy pass.

## Initial source review coverage

Read the bootstrap geometry/formatter/dual-mount requirements and examined:

- formatter capture, runtime staging, typed ext4 profile/configuration, descriptor
  transfer, private helper root/environment, successful execution, failure and Drop;
- source-level ownership of snapshot backing and deferred mapping destruction;
- paired detached-view namespace identity checks, confined `/mnt` attachment,
  and ownership of the root/output namespace handles through attachment;
- direct sealing's use of devmap-snapshot wire codecs, extent boundaries, retained
  recorded zero writes, single data hash stream and immutable reader authority;
- the distinction between formatter success, command acceptance, clean storage
  finalization, sealing and future publication/inventory.

The R1 discovery makes this a non-clean review. Full acquisition/transfer/root
attachment/process teardown, pair finalization, generic API/typestate/style and
requirements-to-evidence review remain before starting a clean streak. Repeated
VM runs are regression evidence, not adherence passes. M6 publication/inventory,
M7 distro/architecture/confidential-platform qualification and M8 retirement are
not inferred from these tests. Legacy code and exploratory evidence remain intact.


## Full clean pass 1 — requirements and ownership first

Frozen implementation: `f8ad64d`; exact source/design/build/fixture hashes are in
`preservation/m5-review/scope.json`. Ledger-only changes do not alter this scope.
This is a self-review, not independent security qualification.

1. Requirements: traced DESIGN.md bootstrap rules and SANDBOX-DESIGN.txt sections
   3–8 through the actual verified-tool fixture and production APIs. Output is a
   writable snapshot over a 64GiB dm-zero origin; filesystem size is independently
   validated and passed as 4096-byte blocks. Captured trusted formatter runtime,
   explicit ext4 profile, cleared environment and private assigned device are
   used. No image import/conversion or recipe-provided privileged formatter args.
   The tool overlay persists across its own steps, contributes no inherited
   layer to output, and is discarded after finalization. Immutable input layer
   data remains cache-backed. Both roots remount and receive scoped policy for
   every command. Context/control/backing descriptors do not enter commands.
2. Acquisition and transfer: traced namespace creation/pinning, both mounts,
   both filesystem pins, domain creation, policy installation and helper launch
   through StoragePreparation. Partial second mount/pin failure retains acquired
   resources. Before admission, Step checks both roots against its bound policy.
   The versioned packet enforces three CLOEXEC descriptors in tool/output/userns
   order. Both idmapped views share the namespace; confined no-symlink/no-submount
   `/mnt` resolution is repeated at attachment. Partial attachment owns both the
   attached tool and detached output. The restricted command has no mount/control
   descriptors or SYS_ADMIN; these restrictions remain the accepted M4 path.
3. Teardown and acceptance: reviewed cancellation, reaping, domain drain/removal,
   policy/identity cleanup, output-then-tool unmount and retry states. The original
   setup/execution/teardown cutoffs are retained, with no retry renewal. Unfinished
   command owners retain storage; R1 now also retains unfinished formatter inputs.
   ExecutionStorage dispatch cannot manufacture a receipt: acceptance is private
   and requires this command's exit success plus both snapshot health checks.
   Failed outcomes stay failed through cleanup retries. Sealing requires detached
   COW authority, trims only unused trailing capacity, uses snapshot wire codecs,
   preserves recorded zero writes and hashes the retained bytes. Read-only sealed
   cursors expose neither write access nor raw descriptors. Low-level sealing is
   deliberately separate from command-success and future publication authority.
4. Rust style/safety: reviewed changed modules and their generic interfaces.
   Sealed marker states and associated resource types prevent invalid public
   transitions; invariant-bearing fields remain private. Shared preparation and
   execution traits have two concrete storage implementations, not parallel
   single/pair command state machines. Internal cleanup enums own distinct partial
   resource states; Options denote optional output/deadline or destructor slots.
   Immutable formatter descriptions, consuming resource transitions and fallible
   explicit cleanup are used. ManuallyDrop retention is safe Rust for unresolved
   lifetime ownership, not unsafe field extraction. No new application unsafe,
   production freestanding utility functions, synchronization sleeps or dependency
   changes were introduced by M5. Cargo and lib.rs still forbid unsafe Rust.
5. Evidence and preservation: checked assertions as well as pass markers. v176
   covers partial pair acquisition and busy rollback; v178 covers aggregate setup
   failures; v180 covers nonzero/timeout and successful/failed busy finalization;
   v181 covers exact metadata; v183 covers formatter timeout/exhaustion; v184/v185
   cover abandoned formatter owners; v186 runs the complete normal suite on R1's
   corrected implementation. Recomputed all recorded hashes and checked verdicts
   for those runs, including external kernel/initramfs/binary artifacts. All 26
   changed source files present in v186's source archive match the working tree;
   the remaining changed file is lib.rs's formatter module export, separately
   covered by the frozen scope. Current host evidence is 56 library tests,
   49 doctests and strict Clippy. No redundant test run was counted as review.
   The external preservation archive exists and no preserved legacy/validation
   paths were deleted since M4's accepted scope.

Evidence boundaries: the fixture uses production preparation, execution,
finalization and sealing, while fixture-owned cache metadata and orchestration
exercise them together. M6 still owns the recipe coordinator, COPY, remaining
stages, networking, inventory and atomic OCI publication. This review does not
claim the binary is a completed builder. Distro boots and amd64/arm64 confidential
hardware qualification remain M7; retirement remains M8. Formatter supervision
covers the selected trusted direct executable, not arbitrary fork-capable tools.
The deployment trusts its kernel and formatters and disposes of the VM if the
supervisor dies; no guardian or cross-invocation recovery is reinstated.

Result: no new finding; clean streak **1/3**. Two further full clean passes are
required before M6. The R1-containing initial review is not part of this streak.


## R2 — stale chain-assembly boundary in the architecture document

The reverse-lifecycle pass traced sealed output back through ReadOnlyChain and
found that ARCHITECTURE.md still said read-only assembly belonged to the carapace
crate and referred to its API as pinned. The current Cargo.toml has no carapace
crate dependency, and src/storage/chain.rs owns the read-only stack through
devmap/lodown. This was an explicit, source-grounded M4 integration refinement:
REIMPLEMENTATION.md's ReadOnlyChain entry explains that the reference API's
returned File/deferred removal cannot provide the required owning partial-setup
and teardown errors. M4's accepted reviews cover that aggregate cleanup behavior.
The architecture document had not been reconciled with that recorded decision.

Correction: document carapace's format/layout authority separately from the
current conglobate-owned device activation/cleanup, retain devmap-snapshot wire
parsing, identify the inspected carapace revision as reference rather than an
active dependency, and link the existing M4 rationale. No executable code,
dependency, format, trust requirement or accepted lifecycle behavior changes.
This is not approval to duplicate wire codecs or to relax explicit cleanup.

Because ARCHITECTURE.md is in the frozen review scope, R2 resets the streak to
zero. The scope.json manifest and pass 1 remain historical and must not be
silently rewritten. A new scope will be frozen before restarting full reviews.
No runtime tests are repeated for this documentation-only correction.

This pass also examined reverse ownership through loop detachment, writable
backing recovery, filesystem flush/unmount, both-volume finalization, command
outcome acceptance, policy-pin release, shared rollback and formatter capture.
The final-state metadata assertions and failure receipts still match the stored
VM evidence. That partial review does not count as a clean pass after R2.
