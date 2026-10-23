# M4 adherence review — findings in progress

Reviewed baseline: c808577. Self-review with findings, not an accepted adherence
pass. Clean streak: **0**. No advancement to M5.

Normative scope: DESIGN.md sections 1, 6, 9 and 12; SANDBOX-DESIGN.txt sections
1–4, 7, 9–10; ARCHITECTURE.md ownership/error/lifecycle requirements; the M4
criteria and mandatory review categories in REIMPLEMENTATION.md.

M4 covers the single-volume privileged lifecycle. Direct bootstrap formatting
and dual volumes remain M5; COPY, enabled networking/resolver injection,
CARAPACE and pipeline/publication remain M6; distribution boots and actual
confidential-platform qualification remain M7. These boundaries cannot excuse
contradictions in M4 foundations. The public build coordinator is still absent.

## Findings

| ID | Source-grounded finding | Required correction |
| --- | --- | --- |
| R1 | HelperProgram, CommandReceiver and gated helper launch inherited stdout/stderr. RestrictedCommand's descriptor audit exempted descriptors 0–2. Redirecting supervisor diagnostics to a regular file exposed that file descriptor to commands. | Supply only bounded diagnostic pipes to commands; trusted code forwards bytes and propagates failures. Test pipe identity with supervisor output redirected to a regular file. |
| R2 | Volume::unmount performs syncfs and normal unmount, but finalization/acceptance never queries SnapshotMapping::usage. Invalid/exhausted snapshot status is tested independently but is not part of acceptance. | Check snapshot status while retaining finalization ownership and the original deadline; reject invalid/exhausted status. Test the integrated connection, not only the parser. |
| R3 | CommandReceiver reported completion before the production caller closed its mapped view and restored its namespace. Later local cleanup failure could follow an already-reported success. | Complete fallible local view/namespace cleanup before reporting. Helper failure in that interval must prevent a success receipt; supervisor teardown remains mandatory. |

R1 experiments v146/v147 are preserved failures. Initial relay placement used
threads in the namespace launcher after new-PID namespace setup. Runtime showed
startup peer loss, not the precise inner errno. Moving the relay to the outer
receiver avoids that thread/namespace placement. v146 also exposed fixture
diagnostics printed only on success; forwarding now occurs on either outcome.

## Observations and remaining review

- Policy activates after restricted-leaf readiness. Teardown cancels the domain,
  reaps the helper, drains adopted children and removes the record before policy
  removal and identity release. Unfinished protection/storage stays retained
  for VM disposal. Integrated evidence supports this ordering, not a destructor
  guarantee or full platform qualification.
- Namespace entry, pivot/detach, five-set capability verification, syscall
  restrictions and confined runtime targets are implemented. SYS_ADMIN is absent.
  Descriptor review nevertheless found R1.
- Verified immutable layers remain cache-backed, with format-required exceptions;
  writable backing is tmpfs-owned. Explicit unmount retains failure ownership.
  Receipt review found R2 despite independent storage primitive tests.
- Active Rust forbids unsafe code. Sealed state traits, private fields and
  consuming transitions protect resource invariants. Internal cleanup enums
  represent encountered failures. Broad public low-level interfaces and retained
  guardian APIs still need final visibility/retirement review.
- Legacy source and preservation evidence remain intact. Failed/passing VM runs
  retain source/logs and coverage limits. amd64 VM tests do not qualify arm64,
  TDX, SEV-SNP or CCA. Not every new acquisition failure is independently injected.
- Finish the source/evidence matrix, dependency/license checks, blocked-I/O and
  deadline coverage, metadata compatibility boundaries, API visibility and current
  checks. This partial review is not one of the three clean passes.

Record fixes below. Relevant edits reset the streak; a clean sequence must review
the complete frozen M4 scope.

## R1/R3 corrections in progress (v148–v150)

The outer contained receiver now gives the namespace-entry child piped stdout
and stderr. Two trusted receiver threads forward bytes with bounded streaming
buffers; they remain in the receiver's cgroup and outside the command PID
namespace. Namespace transitions/spawn happen before these threads. Normal
completion joins the relay after the child exits; relay failure rejects helper
completion. Diagnostic locks are held per write, never across a blocking read.
Failure paths are exit-only in this helper; the supervisor owns domain cancellation.

The production receiver closes its view and restores its private namespace
before reporting command completion. v148 passes normal execution with command
assertions that stdout/stderr are FIFOs while the supervisor output is redirected
to a regular file. v149 adds an injected local completion error after successful
shell execution; the build remains rejected after full cleanup/retry.

R2 remains open. The review is incomplete and the clean streak remains zero.

- v150 supervisor-death regression passes with the corrected relay. Host tests,
  root/fixture Clippy, formatting and evidence hashes pass. R2 and the remainder
  of the review remain open; clean adherence streak is still zero.

### R2 finalization gate (in progress)

Finalization now observes snapshot status after successful flush/unmount, and
checks the original teardown deadline again after that observation. Invalid or
unreadable status retains the unmounted volume, primary helper status, reaper
and original cutoff in the owning failure. Retry repeats the health check;
cleanup alone cannot bypass it. Host tests and strict root Clippy pass.

The existing disposable-VM integration suite is being rerun as v151. A dedicated
exhaustion regression through command acceptance remains required; the existing
snapshot-probe exhaustion test alone does not demonstrate that connection.
R2 remains open pending that evidence. No clean adherence pass is claimed.

v151 completed successfully. Logs, changed implementation source and host check
logs are preserved with verified hashes. The source snapshot includes a later
documentation-only line wrap in FinalizedVolume's comment; executable logic is
unchanged from the booted build. The dedicated negative acceptance test is still
outstanding.
