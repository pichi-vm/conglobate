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

### R2 integrated rejection demonstrated (v152–v153)

v152 failed before exhaustion coverage: the fixture selected an ext4 block size
smaller than the underlying device permits. Preserved the failure and corrected
the fixture to 4096-byte blocks, keeping its filesystem at 16 MiB.

v153 passes the complete normal suite. A successful command first reaches the
existing busy-unmount fixture after syncfs. The trusted fixture then exhausts the
32 MiB COW store by writing into the tail of a 64 MiB device, beyond its 16 MiB
filesystem. After releasing the busy holder, finalization returns InvalidData,
preserves the successful command outcome and removes the mount. Neither volume
extraction nor a further cleanup retry produces an acceptance receipt. This
exercises the production finalization/acceptance connection, not only status
parsing. Commands never receive the device descriptor.

R2 is addressed. Fixture Clippy/format checks and evidence hashes pass. The
runner now requires the new marker; this assertion was added after v153 and
checked against its preserved serial output. Its booted runner is preserved.
Remaining source/evidence review is still incomplete: clean streak remains zero.

### Cancellation/deadline source and evidence review (v154)

Reviewed Step teardown/finalization, DomainWatch/cgroup cancellation,
DescendantDrain, ChildReaper and DiagnosticRelay. The production teardown checks
one absolute 30-second deadline between ownership transitions and retains its
phase on failure. Cgroup populated=0 precedes helper wait and adopted-child
drain; policy removal and identity release follow those gates. DomainWatch polls
the same open cgroup.events descriptor and rereads state, without synchronization
sleeps. ChildReaper uses waitpid/__WALL plus pidfd observation; it retains the
original process identity and collected count on failure. Low-level empty-domain
or ECHILD observations can return after their deadline, but the enclosing
Teardown transition checks the unchanged cutoff before accepting completion.
Finalization likewise rechecks after snapshot status. This supports bounded
acceptance; it does not guarantee a synchronous kernel call can be interrupted.

v154 adds a blocked-diagnostics regression: the fixture retains but never reads
the outer helper stdout pipe. The command marks output-started, then writes more
than pipe capacity. Under its one-second command timeout, execution cancels and
drains the helper/descendants. The output-completed marker is absent. The timeout
outcome survives the existing busy-unmount cleanup failure/retry; no success
receipt is issued. Records/pins are absent and storage finalizes normally after
releasing the busy holder. The complete normal suite and fixture Clippy pass.
This covers interruptible pipe backpressure, not uninterruptible kernel I/O.

API review also confirmed RegisteredGuardian has no production call site;
GuardianWatch is used only by that dormant API and its unit tests. These remain
obsolete exports from the superseded deployment design and need removal from
the active implementation with preserved source/test provenance. No new guardian
should be introduced. Other low-level entrypoints still need final visibility
review; their documented trusted-caller preconditions are not command authority.

Host dependency declarations are recorded in m4-review/host-dependency-licenses.json:
198 resolved packages, all with SPDX license declarations. This is an inventory,
not a completed native-library/distribution license audit. Unfiltered offline
metadata failed on an uncached Android-only package; host-filtered metadata
succeeded. Native libbpf dependencies and appliance packaging remain to inspect.
No clean adherence pass is claimed.

### Superseded guardian API retirement

Removed RegisteredGuardian/GuardianStop, GuardianWatch/ShutdownTrigger and the
unused upstream-watched launch/registration wrappers. Source search showed no
production or current integration-fixture callers; their only remaining uses
were their own unit tests. Removed those two unit tests and the guardian-specific
compile-fail example along with the APIs. Active comments now describe the
supervisor/helper relationship. Normal startup identity exchange, helper
supervision, admission and cancellation remain intact.

Exact pre-removal files, including all tests, are retained under
retired-guardian/ with a baseline commit and verified SHA-256 manifest. Root
unit/doc tests (46 compile-fail examples), root Clippy and integration-fixture
Clippy pass; logs are preserved there. No fresh VM coverage is claimed for this
removal. Previous runtime evidence remains applicable to the unchanged production
path, while whole-milestone acceptance still requires the remaining review.

### Storage source inspection

Inspected Workspace/Scratch, PreparedScute, ReadOnlyChain ownership and
Volume::mount_ext4/unmount transitions. Workspace uses detached private tmpfs,
no executable/device/suid semantics and anonymous scratch files, without a disk
fallback. PreparedScute keeps an opened cache inode for aligned raw data and
creates verification metadata separately; compressed/partial data materializes
into workspace scratch with size/root validation. Later kernel reads require
verity. Chain ownership releases layers top-first. Mount creation acquires its
root descriptor before attachment and transfers ownership infallibly after
move_mount; successful completion requires syncfs and normal unmount, with
failures retaining state. Trusted parent/procfs/initial-superblock assumptions
remain part of supervisor setup, not guarantees inferred from an arbitrary path.
This inspection does not replace metadata qualification or full error-path review.

### Command-boundary source/evidence matrix

Recorded the namespace/root/descriptor/capability/seccomp/runtime/label-policy
inspection in m4-review/command-boundary.md with source hashes. No contradiction
found in that component. Runtime claims are tied to existing amd64 fixtures;
full syscall-profile compatibility, dual-mount metadata and arm64/CC coverage
are not inferred. This component review is not a full clean adherence pass.

### R4: retain policy directory after setup expiry

Acquisition/rollback inspection found a post-directory deadline failure that
returned Pending::None after best-effort directory Drop. Corrected it to retain
Pending::Directory and explicitly close it during cleanup, preserving the owner
and diagnostics on removal failure. No links were attached at this point; the
unpinned object can close normally. Detailed ownership trace and coverage limits
are in m4-review/acquisition-rollback.md. This finding resets the full clean
streak to zero. No exhaustive fault-injection claim is made.

v155 normal VM suite, root tests and strict Clippy pass after R4 correction.
Evidence/source/check logs are preserved with verified hashes. R4 is corrected;
its specific post-creation expiry boundary remains source-reviewed coverage.
