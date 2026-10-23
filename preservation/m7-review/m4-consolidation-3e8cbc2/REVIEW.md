# M4 protocol review and consolidated acceptance scope

Baseline: 3e8cbc2. No production changes. This is a partial source review and
an acceptance checklist, **not a clean adherence pass**. M4 remains 0/3.
The immediately preceding NAT discussion made no implementation progress;
this continuation inspected authoritative source and recorded the next actions.

## Protocol/process/anchor observations

- ProcessObservation uses pidfd readiness; channel closure does not establish
  exit. Its observation does not authorize resource release or replace reaping.
  Readiness errors propagate. A previously exited process may be observed after
  a deadline; enclosing lifecycle acceptance must still check its cutoff.
- Supervision separates pairing, joining and admission with sealed states.
  joining::Packet validates packet lengths, opcodes, descriptor counts,
  truncation and terminal wait-status representation. Received rights are
  close-on-exec. Unexpected traffic fails the phase-specific decoder.
- VolumeInput is explicitly trusted helper input, not authentication of an
  arbitrary external sender. Root/output directory and namespace checks do not
  certify arbitrary descriptors safe. The supervisor owns the private channel
  and backing storage; transfer is not permission to release storage.
- The helper's production completion path closes resolver scaffolding and its
  view and restores its mount namespace before reporting completion. The parent
  still cancels/drains descendants and finalizes storage separately. A reported
  command status is not a clean-storage receipt.
- RecoveryDirectory retains an exclusive root-owned mode-0700 tmpfs directory
  lease. Namespace identity and attached mount checks constrain its use. Arc
  sharing extends the lease; no public early-unlock/raw-descriptor capability
  was found in that owner. Privileged environment replacement is outside its
  cooperative lease guarantee.
- Methods and owned sealed states carry authority. VolumeInput's optional
  output/network fields describe input variants, not command acceptance flags.
  No new production defect was established in this subset. This does not claim
  that all public APIs or every process protocol were independently audited.

## Whole-M4 acceptance checklist

All rows require source, ownership/style, error-path and evidence review in
**each** full adherence pass. Three subset reviews cannot be added together
and called three passes. Source hashes bind this baseline; relevant changes
reset the streak. Passing a test is evidence, never an adherence pass.

| Requirement | Source scope | Evidence and remaining work |
| --- | --- | --- |
| Dedicated VM, trusted kernel/environment policy; no conglobate policy registration | SANDBOX 1, 4; execution, isolation/labels, deployment validation | Check environment admission and exact-name policy controls; keep independent appliance shutdown qualification distinct from fixture poweroff. |
| Private RAM workspace and verified cache-backed chain | SANDBOX 3; workspace, cache/chain, storage, loopback, mapping, verity | cache-storage-v2 covers prepared scutes and mutation, not a full multi-scute chain. Migrate the historical two-scute chain fixture and test composed ordering/invalid headers. |
| Mount ownership, idmapping, no backing/control/context exposure | SANDBOX 4, 7; filesystem, isolation, storage/runtime, execution/helper | confinement-v6 covers actual command boundaries; review all authority transfers and root-target confinement. Metadata and dual-root bootstrap remain coupled M5 requirements. |
| Capabilities, seccomp, namespaces, descriptors, runtime mounts | SANDBOX 7, 9; privileges, syscalls, namespaces, runtime/devices, receiver | confinement-v6 includes actual denial probes with controls. Historical direct-clone mask evidence is not current maintained coverage. Reconcile/migrate it; Arm remains unqualified. |
| Finite command timer; fixed setup/teardown cutoffs | SANDBOX 7.6, 10; cutoff, preparation, step/setup, teardown | Formatter/setup/command failure fixtures and blocked-io-v2; check cutoff propagation, no renewal and no sleep synchronization. |
| Stable process authority, admission, descendants and diagnostics | SANDBOX 10; process, domain, anchor, execution | Contained/join/gate/attached-exit fixtures; verify pidfd observation, child wait ownership, cgroup quiescence and reaping independently. |
| Partial setup and retained recovery ownership | SANDBOX 10; preparation/rollback, control_record, domain, retention | Publication, namespace, mount, launch and join failures. Inspect each acquisition edge and record mismatch; never infer cleanup from absent records alone. |
| Writeback/finalization errors reject output even after cleanup retry | SANDBOX 10–11; storage finalization, step/execution, pipeline rejection | writeback-v3, busy-finalization-v2, busy-execution-v1 and rejection-only retry rootless tests. Preserve first and subsequent diagnostics; trusted status fixtures are not image-command coverage. |
| Blocked I/O and supervisor death | SANDBOX 10.5; cancellation, retention, deployment contract | blocked-io-v2 and blocked-supervisor-death-v1; independent fixture poweroff only. No claim that the real appliance controller has been qualified. |
| Network teardown and resolver ownership | SANDBOX 8; network, resolver, preparation, execution/helper | Existing NAT/DNS/nonzero/timeout/composite evidence; IPv6 and unsolicited inbound validation remain open under M6/M7. Scope interfaces do not waive those gates. |
| Safe Rust, meaningful owners, sealed typestate, consuming recovery | All above modules and public callers | Review full API surface including inference-based consumers. No generic abstraction or freestanding helper solely to reduce line count; RAII is not a fallible-cleanup success receipt. |
| Evidence fidelity and preservation | Maintained harnesses, assessments, source snapshots, milestone ledger | Compare changed sources before reusing evidence. Large /var/tmp artifacts remain unarchived. Keep failures and legacy code; migrate GuestRuntime before M8 retirement. |

## Historical review reconciliation

historical-actions.json preserves the 46 accepted/open entries from the original
111-ID reconciliation ledger. These are **not** 46 current defects: subsequent
fixes, dependency/policy removal and rejected proposals change dispositions.
The old ledger even requests a clone errno later withdrawn by review consensus.
Do not implement it literally or claim it is fully resolved. Reconcile each ID
against parent-evaluation.md, later review consensus, current code and tests.
For M4 prioritize API consumers, deadline-free entry wrappers, diagnostic
ownership, proc exposure, formatter authority and namespace-mask coverage.
Formatter tools are trusted; an old recommendation to restrict them is not
permission to silently change the agreed trust model.

## Ordered next work

1. Migrate and run the full two-scute chain/invalid-header regression in the
   maintained disposable-VM harness. Existing generator includes these inputs.
2. Reconcile M4-related historical actions, especially current clone-mask and
   proc evidence, without treating obsolete registration findings as live code.
3. Perform a complete M4 review across every checklist row. Record findings
   and fixes; only a complete clean pass increments the streak. Repeat twice
   on unchanged scope after the first clean pass.
4. Keep M5/M6 reviews and qualification gates open, including current distro
   builds/boots, architecture/confidential platforms, CI and durable retirement.
   No checklist row relocates or waives an objective requirement.
