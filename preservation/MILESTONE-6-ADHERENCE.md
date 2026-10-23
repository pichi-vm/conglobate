# Milestone 6 adherence review

Status: **0/3** clean passes. Requirements/evidence audit in progress; no full
clean pass has been claimed. The complete scope is DESIGN.md, SANDBOX-DESIGN.txt,
ARCHITECTURE.md, latest user decisions and the M6 row in REIMPLEMENTATION.md.
Tests are evidence, not adherence passes. These are self-reviews unless stated.

## Initial requirements/evidence map

- Pipeline combinations and ancestry: DESIGN sections 5–6; pipeline bootstrap,
  carapace and firmware coordinators; v250 CLI matrix and reopened sealed outputs.
- COPY: source/context confinement, Docker reference modes/patterns and mounted
  metadata; source/destination code and v250 inherited regression. DESIGN section
  7 scopes build-variable interpolation to shell and DTB templates; it does not
  require dynamic COPY paths. Review destination/account traversal and error
  ownership before a clean pass.
- Firmware and DTB: paired extraction, disposable tool ancestry, CARAPACE,
  transformed measurements and typed/default operations. Source review and
  evidence cross-check remain required for every branch, not only positive cases.
- Networking: v250 all-stage CLI TCP/resolver/inventory and failed publication;
  v251 composite acquisition failure and expired cleanup retries; v252 forwarded
  IPv4/IPv6 unsolicited TCP rejection plus policy-removed positive controls,
  existing UDP/TCP/DNS tests. Environment forwarding prerequisite needs explicit
  documentation/qualification. R1 is addressed below.
- Output: v250 ORAS copies all 12 successful variants byte-for-byte. Review atomic
  no-replace publication, all inherited components and inventory binding. DESIGN
  section 11 now specifies the output directory itself as the OCI layout,
  resolving its previously proposed wrapper illustration in favor of the user's
  explicit OCI-directory requirement. No wire format or implementation changed.
- Rust style/safety: inspect owning methods, sealed state traits, consuming
  transitions, retained failures and cleanup deadlines throughout M6 changes.
  No completion claim follows from Clippy or a search for unsafe/sleeps alone.
- Preservation and qualification: legacy/evidence retained. M7 distro/platform
  qualification, CI and M8 retirement remain separate outstanding milestones.

## R1 — address allocation observation is not serialized with assignment

Addressed by the correction below; no full clean pass claimed.
`CommandNetwork::prepare` in `src/network/setup.rs` captures topology and
selects interface names/subnets, then creates a namespace, creates the pair, and
configures addresses. Exclusive pair creation safely rejects a stale identical
interface name, but it is not a subnet reservation. A second builder can observe
an already-created first pair before that pair has any addresses, select a new
interface name and the same free subnet, then configure overlapping routes.
`NetworkTopology` correctly documents itself as an observation, not a reservation;
the composed production setup currently supplies no reservation or serialization.

SANDBOX-DESIGN.txt 3.2 requires concurrent builders not to collide or remove each
other's resources. The dedicated-VM deployment does not justify ignoring that
explicit ownership requirement. Serialize the observe/create/configure interval
per supervisor network namespace, or acquire an equivalent atomic reservation.
Use bounded/fail-fast acquisition and an owned guard; no synchronization sleeps,
random-collision assumptions or recipe-visible control. Validate competing setup
and release/failure paths in the disposable VM before considering R1 resolved.

No full clean adherence pass has started. The correction and focused evidence
below address R1; the three consecutive complete reviews remain required.

### R1 correction and evidence

`AllocationLease` owns a CLOEXEC file descriptor for `/proc/self/ns/net` and a
nonblocking exclusive flock on its namespace inode. Production setup acquires
it before topology observation and retains it through successful setup or error
return. Independent opens/processes in the same network namespace share this
kernel lock even with distinct mount namespaces; unrelated network namespaces
use different inodes. Contention fails before helper creation or network mutation.
No shared filename, clock-based retry, synchronization sleep or random-address
collision assumption is involved. Normal Rust field destruction releases the
lease, and CLOEXEC prevents helper execution from retaining it. All failed setup
states perform cleanup only, never resume configuration after releasing the lease.

- v253 establishes kernel flock exclusion across independent opens/processes and
  release on close, before production used it.
- v254 exercises production setup under contention and proves no child/link/control
  marker was created. It also checks cross-mount-namespace exclusion, independent
  network namespaces, and a second successful network with a distinct live /30
  prefix while the first remains active. Second-network teardown preserves the
  first. Missing-parent/NAT-collision cleanup and expired-deadline tests still pass.
- Strict root/fixture Clippy pass. Sources, logs and provenance are preserved.

These are focused correction evidence, not a complete M6 adherence review.

## R2 — command stdout bypassed the CLI logging contract

DESIGN section 2 requires logs on stderr. The shared trusted-helper launcher
inherited stdout, so image command output (and formatter stdout) escaped through
the public CLI stdout. The v249/v250 nonzero-command fixture confirmed that old
behavior by observing its command marker on stdout.

Correction: the helper command factory duplicates stderr as its
stdout before spawn, covering formatter and recipe helper roles without changing
private stdin/control transport. Image shell redirection retains normal meaning.
The CLI matrix now asserts empty stdout for success and failure, verifies markers
from all three enabled recipe stages on stderr, and observes the failed-command
admission marker there too. Full VM/ORAS regression v255 passed, along with root
and fixture strict Clippy. Sources, logs and provenance are preserved in
validation/rewrite-command-integration/evidence/v255. R2 is addressed; this is
focused correction evidence, not a complete adherence pass.

## Additional source inspection during R2 validation

Read firmware coordinator selection and disposable snapshot execution/extraction,
COPY source/destination authority and metadata application, DTB property/template
execution, and output assembly/publication entrypoints. Checked separately selected
tool/output ancestry, paired extraction after writer quiescence, absent CARAPACE,
property-exists/default skip behavior, no-match replacement, preservation of exact
unchanged DTB bytes, and no public mutable staging escape from AssembledImage.
COPY runs under exclusive quiescent storage authority; image symlinks are resolved
against that root and created leaf files use exclusive/no-follow opens.

This is an incomplete review, not a clean pass. Remaining source/evidence coverage
includes the rest of assembly/inventory/publication, bootstrap coordination,
network setup/transport/finalization, and the shared execution changes, followed
by the complete style/safety/preservation audit. Firmware extraction can mask its
primary capture error when cleanup also fails; inspect this diagnostic path before
acceptance. Successful publication remains blocked in that case.

## R3 — firmware extraction diagnostics lost on cleanup failure

Addressed: FirmwareTools::run now captures cleanup as a Result and combines it
with extraction before finalization. Both failures remain in the diagnostic;
either failure still prevents finalization/publication. Cleanup retains its
original order and stops at the first failed teardown operation. The mounted
borrow is consumed before closing the active chain; no additional mutable owner
or state flags were introduced.

Strict Clippy and all-target tests pass. Evidence and the initial borrow-check
failure are preserved in evidence/firmware-diagnostics. The simultaneous kernel
extraction/cleanup failure was not fault-injected; the exhaustive Result match
is source evidence for combined diagnostics, not a new kernel qualification.

Also documented the existing IPv4/IPv6 forwarding prerequisites in DESIGN and
SANDBOX-DESIGN. v250 already tests refusal with either disabled. This does not
claim that all deployment environments provide those prerequisites.

Continued output audit: reviewed StagedDirectory construction, verified streaming
writes, metadata writes and renameat NOREPLACE publication. The parent descriptor
anchors staging and destination; no fallible work follows successful publication.
Existing tests cover late destination collisions and mutated/truncated blobs.
Reviewed firmware assembly preservation of config and non-firmware layers,
separate tool ancestry, paired replacement and parent-chain checks. Remaining
assembly branches, inventory and coordinator audit still prevent a full clean
pass; the streak remains zero.
