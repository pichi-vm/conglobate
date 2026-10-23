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
  documentation/qualification. R1 below remains open.
- Output: v250 ORAS copies all 12 successful variants byte-for-byte. Review atomic
  no-replace publication, all inherited components and inventory binding. DESIGN
  section 11 still labels an `image.oci/` wrapper illustration as proposed, while
  output is directly an OCI layout. Reconcile this documentation explicitly
  against the user's OCI-directory requirement; do not quietly change a contract.
- Rust style/safety: inspect owning methods, sealed state traits, consuming
  transitions, retained failures and cleanup deadlines throughout M6 changes.
  No completion claim follows from Clippy or a search for unsafe/sleeps alone.
- Preservation and qualification: legacy/evidence retained. M7 distro/platform
  qualification, CI and M8 retirement remain separate outstanding milestones.

## R1 — address allocation observation is not serialized with assignment

Open. `CommandNetwork::prepare` in `src/network/setup.rs` captures topology and
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

No full clean adherence pass has started. R1 must be fixed and affected evidence
updated before the required three consecutive complete reviews.
