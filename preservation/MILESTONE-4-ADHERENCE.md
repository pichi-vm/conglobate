# M4 adherence gate

Scope: implementation at ed0fecd408562aed8bc083e8fbe412c4b80ec514,
with exact source/specification/fixture hashes in m4-review/accepted-scope.json.
Current streak: **2**. M4 is not accepted; M5 must not start yet.
These are self-reviews, not independent external reviews.

## Pass 1 — requirements through lifecycle, clean

Reviewed the M4 criterion and mandatory categories in REIMPLEMENTATION.md against
SANDBOX-DESIGN.txt trust/storage/policy/command/lifecycle requirements and the
architecture's ownership and dedicated-VM contract. Reconciled the component
reviews in MILESTONE-4-REVIEW.md and m4-review/{command-boundary,acquisition-rollback}.md
with the frozen source; inspected storage mapping/loop ownership, mount idmapping,
policy activation/disable, setup/execution/teardown and acceptance transitions.
Checked current source hashes for the earlier command-boundary inspection.

1. Requirements and scope: native ext4/idmapped mounts, verified cache-backed
   layers, tmpfs mutable storage, mapped-root command namespaces, scoped foreign
   labels, no command SYS_ADMIN, finite deadlines, cancellation and independent
   VM disposal are implemented for the single-volume lifecycle. No pichi runtime
   or guardian dependency remains. Initial filesystems/mount targets and deployed
   kernel facilities are trusted setup prerequisites. Dual-volume formatting and
   sealing are M5; COPY/network/CARAPACE/publication are M6; real distro/platform
   qualification is M7. These pending deliverables remain explicit, not inferred
   from the sandbox fixture.
2. Rust style and state invariants: sealed generic states protect mount/device/
   step transitions; consuming methods move ownership. Immutable policy/input
   descriptions are distinct from cleanup state enums. Those private enums hold
   acquired resources and permit cleanup-only retries. ManuallyDrop/Retained keep
   unfinished protection alive for VM disposal; they do not assert cleanup success.
   No freestanding production Rust functions in src, unsafe application Rust or
   synchronization sleeps found. Entrypoint build.rs is the allowed exception.
3. Error and lifecycle review: mapping activation errors retain leases; immediate
   close reports busy rather than accepting deferred removal. Loop detachment
   checks unbound status. Commands receive mapped views, not backing handles.
   Setup and rollback retain partial acquisitions; R4 now retains an empty policy
   directory after expiry. Ready/start gates precede image execution. Cancellation,
   helper reaping and descendant drain precede policy/identity release. Finalization
   requires flush, ordinary unmount and snapshot health under the original cutoff.
   Command failure survives cleanup success; private receipt construction prevents
   pairing an unrelated successful result with storage. Synchronous kernel calls
   can outlast a cutoff; late completion is rejected, not claimed interruptible.
4. Evidence and limits: v155 normal suite covers successful sequential commands,
   setup/launch failures, nonzero exit, timeout, blocked diagnostic pipes, snapshot
   exhaustion and cleanup retry. v156 reruns abrupt supervisor death on current
   code and verifies retained protection until independent VM shutdown. Host tests,
   strict Clippy and formatting passed for the frozen implementation. Recent
   evidence/provenance hashes (v150,153–156) and guardian-preservation hashes were
   verified. Exact post-directory deadline timing is source-reviewed, not a
   separately injected case. Pipe backpressure does not establish D-state task
   cancellation. No arm64/TDX/SEV-SNP/CCA or distro compatibility claim is made.
5. Preservation: original baseline tag remains 7bd1863; reference implementation
   and exploratory evidence remain retained. Removed superseded guardian files
   and tests are recoverable verbatim with hashes. No M8 retirement occurred.
   Host Cargo license declarations are inventoried; native/appliance distribution
   audit remains packaging/qualification work, not a claim of this pass.

R1–R4 are corrected. No new finding in this pass. This is one clean review,
not milestone acceptance. The next pass must revisit the frozen scope in a
different order and cover all five categories again.

## Pass 2 — acceptance back to acquisition, clean

The same 104-file frozen scope is byte-identical. Reviewed in reverse lifecycle
order: FinalizedVolume/ExecutedVolume and CommandResult; finalization retries;
helper cancellation, reap and record cleanup; completion reporting in helper
dispatch; admission and execution cutoff; verified storage dependencies and
namespace mapping. Reconciled these with the policy/command/acquisition reviews
and normative M4 requirements rather than counting another test execution.

1. Requirements: acceptance is one successful command plus its own fully finalized
   volume, not shell exit or generic cleanup success. Helper completion follows
   local view/namespace cleanup. The supervisor's finite command deadline still
   governs receipt observation even though a reporting helper has its own bounded
   send/wait operations. Default offline namespaces, explicit pipe diagnostics,
   real image IDs through idmapping and scoped labeling match the current M4
   boundary. Full pipeline and distro/platform claims remain excluded as in pass 1.
2. Rust style: receipt fields and constructors remain private, and map_cleanup
   provides diagnostic association rather than a constructor for ExecutedVolume.
   Retry methods consume the failed owner. Mapping/volume/namespace states are
   sealed; the cleanup enums encode actual resource ownership. FailureDiagnostics
   shares immutable concrete errors with Arc and extends ordered history on
   consuming transitions; it does not require shared mutable error state.
3. Safety/error paths: Finalization cannot produce success on timeout, invalid
   snapshot status, failed flush or busy unmount. ExecutionFailure preserves the
   original command result through retries. Domain cancellation closes admission
   channels and stops the subtree before helper reap and process-wide drain;
   policy remains retained until quiescence. Low-level helper statuses are
   diagnostic, not command receipts. SetupCutoff forwards one canonical absolute
   monotonic cutoff conservatively; it does not mint a new helper budget.
   Readonly layers authenticate through verity, using trusted planned parameters;
   snapshot headers are read through that mapping. Chain teardown retains lower
   layers on upper-layer failure. Mapped ID ranges exclude parent root and invalid
   all-ones IDs. No ownership shortcut or new finding identified.
4. Evidence: inspected integrated fixture assertions, not only PASS markers:
   command exit 37, timeout/blocked output, setup/local-completion failures,
   busy unmount and snapshot exhaustion stay rejected; successful command plus
   successful retry alone yields a receipt. Reverified v155/v156 full provenance
   hashes and passed verdicts, both explicitly not full platform qualification.
   Existing unit/compile-fail/Clippy results apply to unchanged source. No repeat
   test run is being counted as a review pass. Uninjected expiry boundary and
   uninterruptible-I/O limitations remain explicitly recorded.
5. Preservation: verified retired guardian originals against their manifest;
   the full implementation scope and reference baseline are unchanged. No legacy
   source, historical evidence or user changes were removed by this review.
   Changes in this pass are ledger-only.

No new finding. Consecutive clean streak: **2 of 3**. M5 remains gated.
