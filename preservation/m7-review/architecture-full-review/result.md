Verified: `CopyDestination::bootstrap` takes the same root/output pair (same layout as execution), and `FirmwareBuild` has no `/mnt` output mount. Findings below.

## M2 architecture adherence review at fc0ad91

I read all 338 lines of ARCHITECTURE.md against DESIGN.md, SANDBOX-DESIGN.txt and REIMPLEMENTATION.md, and cross-checked claims against source. **I do not mark M2 complete.**

### Substantive contradictions

**F1 — DESIGN.md:114 and :125 contradict the disposable-VM requirement (High).**
DESIGN.md §1.1 states *"Conglobate does not require a VM, virtiofs, GPT transport, PID 1, or a particular hypervisor"* and *"The primary conglobate integration tests run without VM infrastructure."* SANDBOX-DESIGN.txt 1.1a makes the dedicated disposable VM with an independent shutdown controller a **deployment requirement**, and ARCHITECTURE.md:302-329 builds its whole lifecycle on it — *"Conglobate is the sole build workflow in a disposable VM"*, with publication refused if cleanup cannot finish.

This is not terminology. Two distinct propositions are being conflated:
- *The binary has no VM/pichi/transport code dependency* — true, and SANDBOX-DESIGN 1.1 states it compatibly (*"does not depend on pichi code, a VM transport, a particular init system"*).
- *Conglobate does not require a VM* — false under the current lifecycle, because the safety argument for abrupt supervisor death depends on the environment stopping the VM.

The user's framing is the correct reconciliation and DESIGN.md §12 already contains it (`:1124-1126`: *"The binary remains standalone and does not create VMs. Privileged execution is qualified in disposable VMs with the independent shutdown contract"*). §1.1 is the stale half. Likewise `:125` is true only of **rootless** tests; every privileged qualification in the current evidence set runs in a VM. **Implication:** a reader of §1.1 alone could conclude the deployment contract is optional, which would void the recovery design. Narrow §1.1 to the dependency claim and scope `:125` to rootless tests.

**F2 — REIMPLEMENTATION.md:141-144 still reads as an unqualified M2 acceptance (Medium).**
*"M2 accepted after initial findings/fixes and three clean adherence passes"* sits in the M2 work log with no reopening marker at that location. The reopening is recorded far away at `:18`, `:32`, `:4163-4164` and `:4379`. The historical record should be retained — but an in-place pointer at `:141` is needed, otherwise the work log contradicts the 0/3 status. **Implication:** the most-likely-read location states the opposite of current status.

**F3 — ARCHITECTURE.md:137 overstates what the type system delivers (Medium).**
*"Public handles must not permit recovering unrestricted writable FDs."* `VolumeView::descriptor()` (`src/filesystem/attachment.rs`, `pub(crate)`) returns a cloned `File`, and `Volume::view()` is `pub`. The invariant holds **today** by visibility, not by type structure — `pub(crate)` is a module-boundary property, and ARCHITECTURE.md:5-6 itself warns that compile-time restrictions do not prove external state. Either qualify the sentence as a visibility obligation or state the enforcing mechanism. **Implication:** low practical risk, but it is the kind of claim that invites a future `pub` widening with no test to catch it.

### Verified accurate (no finding)

I checked rather than assumed, per the "not invented" requirement:

- **All 61 types in the owner table exist in source.** No invented names or modules. The table correctly disclaims (`:71-72`) that original proposed names are not additional requirements.
- **State flows match source.** All seven `Step` markers (`src/step.rs:70-82`); `admit_until`/`cancel_until`/`reap_until`/`drain_until`/`finish_record` present with the documented input→output states; `finish` is genuinely infallible (`src/step.rs:443`, returns `StepCompletion<R>` not `Result`); `teardown` at `src/step/teardown.rs:198`. All four `Supervision` states exist.
- **Ordering claim holds.** `drop(namespace)` occurs at `src/step/teardown.rs:168` inside the `Finishing` arm — after drain — matching §"Error and recovery ownership" step 3.
- **"No activation, disable, policy-close, installation-retry" is true.** Grep finds no such states; the two hits are an unrelated comment and a network test name.
- **Filesystem schema matches exactly:** 16 MiB–64 GiB range, 4096 alignment, B/KiB/MiB/GiB only, 16-byte NUL-free label, `LOGICAL_DEVICE_BYTES = 64 GiB` (`src/value.rs:50,67-70,86-87,102`).
- **Registration-free claims hold:** no policy module, no BPF build deps, `Joined` carries acknowledgment and FD closure (`joining.rs:64,92`).
- **COPY and firmware claims hold:** `CopyDestination::bootstrap(root, output)` uses the same layout as execution; `FirmwareBuild` has no `/mnt` output.
- **Preservation obligations met:** both preserved files verify `OK` against their `.sha256`.
- **All 22 traceability anchors exist**, and the table correctly separates current from historical/unavailable coverage with per-row evidence boundaries.

I specifically did **not** treat open items as architecture failures: receiver start-send coverage, legacy harness migration, distro/platform qualification and M8 durable archive are all correctly *required* by the architecture, which is the right state for them.

### Coverage ledger

| M2 deliverable | Status | Basis |
|---|---|---|
| Module/type ownership | **Adequate** | 61/61 types verified; boundaries stated per module |
| Sealed marker state traits | **Adequate** | Markers + sealed `private` module confirmed in source |
| Immutable plan | **Adequate** | `BuildPlan` immutable; §"Pipeline owners" explicit |
| Consuming transitions | **Adequate** | Transition chain verified; `finish` infallible as claimed |
| Authority handles | **Qualified** | Correct in substance; F3 overstates the mechanism |
| Explicit fallible finalization/recovery | **Adequate** | 6-step ordering matches teardown source; Drop correctly disclaimed |
| Filesystem properties | **Adequate** | Schema matches `src/value.rs` exactly |
| Inventory schema/migration | **Adequate** | Versioned deterministic encoding; decode disclaimed as non-proof |
| Requirements-to-tests traceability | **Adequate** | Anchors exist; current vs. historical separated |
| Rust style obligations | **Adequate** | forbid(unsafe_code), no-util-module, trait restraint, thread-affinity all stated |
| Preservation obligations | **Adequate** | Checksums verify OK |
| Dedicated-VM lifecycle | **Contradicted** | F1: DESIGN §1.1 vs SANDBOX 1.1a |
| Milestone status consistency | **Contradicted** | F2: work-log entry reads as accepted |

**Three findings: one High (F1), two Medium (F2, F3).** The architecture is substantively sound and source-grounded; the contradictions are documentation-level but F1 touches a load-bearing safety premise.

**Scope limits:** architecture review only — this establishes nothing about M4–M8 qualification. No test run here or previously counts as an adherence pass. Native amd64 only in the referenced evidence. Milestone acceptance and the 0/3 streak remain yours; I have made no edits, run no privileged operations, spawned no agents, and proposed no restoration of registration.