# M4 adherence gate

Scope: implementation at ed0fecd408562aed8bc083e8fbe412c4b80ec514,
with exact source/specification/fixture hashes in m4-review/accepted-scope.json.
Current streak: **1**. M4 is not accepted; M5 must not start yet.
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
