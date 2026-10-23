# Milestone 5 adherence review

Status: M5 accepted with **3/3** consecutive clean self-review passes on scope-v2.
The earlier pre-R2 pass and scope remain historical.
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


## Restarted full pass 1 — requirements, lifecycle and public authority

Frozen scope: `preservation/m5-review/scope-v2.json`, implementation b12bfdd.
All 120 hashes match. Only ARCHITECTURE.md differs from the historical frozen
scope; production code and validation fixtures are unchanged. R2's correction
is also reflected in the implementation-order ledger's sealing API wording.
This is a self-review, not independent security qualification.

1. Rechecked DESIGN bootstrap geometry, typed properties, tool/output separation,
   reserved mountpoint, metadata, and direct sealing against the formatter,
   paired storage and fixture. Formatter arguments use validated capacity/label
   as data; captured executable/configuration bytes are the executed inputs.
   Runtime staging is read-only; its private device mount is separate. No image
   command selects a privileged formatter. Network, COPY, inventory and artifact
   publication remain explicitly M6 obligations.
2. Traced acquisition through namespace creation/pinning, pair mount, filesystem
   pinning, domain/policy acquisition, helper launch and transfer. Both roots
   are checked against policy before admission; pair views use the same mapped
   namespace. Partial mount/pin/launch failures retain their remaining owners.
   Reviewed the M5 changes to receiver diagnostics and mountpoint preparation;
   they preserve bounded startup and reject symlink/submount attachment.
3. Traced success and failure through shared execution, teardown, paired unmount,
   snapshot validation and sealing. Private acceptance checks this execution's
   outcome; cleanup cannot turn exit failure into success. Retry preserves the
   cutoff and original diagnostics. Formatter wait/cancel only releases the
   retained input box after successful reaping. Dropped unresolved owners retain
   backing for VM disposal. Direct sealing uses shared wire decoders, retains
   recorded zero exceptions and terminator metadata, and exposes read-only
   readers. Sealing itself does not assert command or publication success.
4. Rechecked sealed storage traits, associated concrete cleanup types, private
   receipt fields and consuming transitions. Partial-state enums carry actual
   resources rather than permission flags. Formatter profiles/capture are
   immutable. Safe ManuallyDrop implements deliberate unresolved-owner retention;
   no unsafe application Rust or synchronization sleeps were added. The shared
   single/pair implementation avoids a second execution state machine. Accepted
   unchanged M4 containment and teardown guarantees remain prerequisites.
5. Compared fixture assertions to claims: two commands remount both roots,
   separate tool/output state, deny labeling on unrelated tmpfs, and reopen the
   sealed chain to check numeric ownership, modes, ACL, capability and foreign
   SELinux labels. Reviewed formatter timeout/cancel/exhaustion/drop assertions
   and failed/successful busy-output finalization. Rehashed every recorded file
   (including external boot artifacts) for v176, v178, v180, v181 and v183–v186;
   all hashes match and verdicts pass. These runs explicitly do not claim full
   sandbox qualification. No new runtime run was needed for unchanged code.

Result: clean, restarted streak **1/3**. Prior R1/R2 findings and historical pass
remain intact. Full recipe orchestration and publication still belong to M6;
representative distro boots, other architectures and confidential hardware are
not proved by this VM fixture. No preservation material was retired. Two further
complete clean passes are required before advancing.


## Restarted full pass 2 — reverse lifecycle and failure evidence

Scope-v2 remains unchanged (all 120 source/design/build/fixture hashes checked).
This separate self-review starts at final output and works backward through
cleanup, admission, preparation and formatter acquisition. No production change
or new runtime test was needed.

1. Requirements and acceptance: compared DESIGN's bootstrap and sandbox rules
   and SANDBOX-DESIGN sections 5–7 against the two-command fixture and production
   APIs. Only the new zero-origin output store is sealed; the verified tool
   origin remains unchanged and its disposable overlay is discarded. Every run
   remounts both assigned roots. Typed ext4 geometry/label, captured formatter
   execution, scoped two-filesystem labeling and no image conversion remain
   consistent. COPY/network/publication examples require M6, not additional M5
   qualification claims.
2. Ownership and failures: traced SealedScute back to detached scratch, confirmed
   loop unbound-state checking rather than treating deferred clear as detachment,
   and checked snapshot-origin borrowing. Volume finalization explicitly syncfs's
   before normal unmount; lazy detach is destructor fallback only. Paired cleanup
   retains output/tool partial states and one original deadline. Step teardown
   drains descendants before removing policy and pins, then both volumes pass
   health validation. Pending failures expose no partial accepted output.
   Reviewed helper attachment failures and their lease fallback: they retain
   mapped namespace owners, while supervisor storage stays independently owned.
   Formatter launch has no live child on failure; wait/cancel retention persists
   until a successful reap. VM disposal remains the unresolved-owner fallback.
3. API/style: checked sealed marker states and private acceptance constructors,
   shared storage-associated cleanup types, immutable formatting inputs and
   consuming transitions. Destructor ownership slots and cleanup enums represent
   remaining resources; none substitutes a boolean for public state proof.
   No new freestanding production utility layer, synchronization sleeps or unsafe
   application Rust. Cargo/lib.rs retain the unsafe-code prohibition. The generic
   execution path checks the original outcome after finalization, including retry.
4. Evidence: read actual assertions in formatter_failures and bootstrap fixtures.
   Timeout checks require the expected Deadline error; cancellation checks an
   empty child list. Exhaustion checks invalid snapshot health. Drop tests check
   retained mapping and deliberately terminate the disposable test process. Busy
   output cases prove extraction fails before unmount and that retry preserves
   exit 17 versus success. Successful two-step and reopened-chain checks cover
   ownership, ACL, mode, capability and foreign labels; no boot-time behavior is
   inferred. Existing logs still show 56 library tests, 49 doctests and successful
   root/fixture Clippy. Evidence hashes were checked in the preceding pass and
   the frozen source has not changed.
5. Preservation and claims: external original archive exists (5,081,763,840 bytes);
   no legacy/preservation/validation paths were deleted since the M4 accepted
   scope. Historical failures and reviews remain. This review does not turn
   fixture-owned orchestration/OCI metadata into production pipeline completion,
   or make distro, Arm or confidential-hardware claims.

Result: clean, restarted streak **2/3**. One further full pass is required. The
summary milestone table is synchronized with this ledger; that status recording
is outside the frozen implementation scope. M6 remains gated.


## Restarted full pass 3 — public API and milestone acceptance

Reviewed unchanged scope-v2 from exported authority and the complete M5 acceptance
criteria. All 120 frozen hashes still match; all 41 archived source/build/fixture
files in v186 match current bytes. This is the third self-review, not an independent
review or production security qualification.

1. Requirements: traced recipe FilesystemSpec through validated capacity/label,
   Ext4Format, captured runtime/configuration, writable zero-origin snapshot and
   paired tool/output execution. Initial ext4 support and fixed 64GiB device
   geometry are explicit. Tool changes persist between commands, not into the
   output chain. Both roots remount with two-target policy. Fixture output is
   sealed directly and reopened through production cache verification/assembly.
2. Public authority: inspected formatter, storage and execution exports and their
   constructors/accessors. Formatting carries live child plus retained inputs;
   FormattedSnapshot proves only reaping/health. Mounted BootstrapVolumes cannot
   be split. FinalizedBootstrap is cleanup proof, while ExecutedBootstrap's
   private construction additionally requires its own successful command outcome.
   SealedScute owns data/tree and exposes read-only cursors. Low-level supervisor
   storage APIs intentionally remain distinct from future publication authority.
3. Failure/ownership: rechecked the shared preparation/execution dispatch, owning
   partial acquisition states, descriptor transfer, private mount attachment,
   fixed cutoffs and non-renewable cleanup retries against the prior two passes.
   R1's release requires a successful reap; unresolved input retention survives
   Drop. Finalization flushes/unmounts both volumes and checks both snapshots;
   failure never manufactures successful execution or publication. Origin and
   workspace borrows plus kernel references retain backing through explicit
   detachment. No guardian, recovery invocation or synchronization sleep added.
4. Rust style: immutable validated descriptions and consuming resource owners;
   sealed state/storage traits and private fields; methods on meaningful types;
   safe library operations under forbid(unsafe_code). Existing compile-fail
   tests check receipt construction and early extraction restrictions. Runtime
   variants own cleanup resources; no new arbitrary shared mutability or state
   flag interface is introduced.
5. Evidence: checked capture mutation/collision tests, typed label arguments,
   missing-loader rejection and actual formatter fixture; then verified reopened
   output assertions for bytes, numeric ownership/mode, ACL, file capability
   without build-root binding and foreign SELinux label. Timeout/exhaustion/drop,
   acquisition rollback and busy finalization evidence remain applicable to the
   unchanged source. Existing 56 library tests, 49 doctests, strict Clippy and
   v186 normal VM pass stand; reviews are not repeated test runs.
6. Preservation/scope: no code/reference retirement. M6 still must connect recipe
   orchestration, COPY, layering, firmware, transforms, networking, inventory and
   publication. The OCI fixture writer is not production output. Package installs,
   distro boots, enforcing target policy and architecture/confidential hardware
   require M7 evidence. The trust contract still assumes the deployment disposes
   of the VM on supervisor death.

Result: clean, streak **3/3**. M5 is accepted at its defined direct-layer bootstrap
implementation boundary. M6 may begin. Later findings reopen the relevant gate;
this acceptance does not complete the overall rewrite.
