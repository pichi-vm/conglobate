# Registration-free architecture adherence

Status: M2 reopened, zero consecutive clean passes. No production code changed.

Initial parent inspection found an underspecified behavioral admission probe.
Source: `src/isolation/root.rs` applies privileges and seccomp in
EnteredRoot::restrict, then RestrictedCommand::start calls the readiness gate.
`src/isolation/receiver.rs` relays readiness and waits for supervisor admission.
These existing transitions supply the ordering; no new process or protocol is
needed. ARCHITECTURE.md now locates a descriptor-based anonymous-inode check
before readiness, with trusted assigned-root layout and explicit scope limits.

Mechanism evidence: global-label-v3 passed O_TMPFILE/f*xattr under zero effective
capabilities. The fixture uses one UID mapping and a chowned directory, not the
production idmapped view or seccomp. Therefore implementation qualification is
outstanding. Architecture edits reset the review scope; this is a finding/fix
record, not a clean pass. Claude's existing review remains in progress.

Preservation: v2 sources/logs are unchanged; v3 sources, guest scripts, hashes,
verdict and logs are separately retained. External runtime artifacts still need
durable archival before M8. No host mount, BPF load or credential change occurred.

## Initial Claude findings and parent disposition

- D1: accept explicit documentation, reject need to retain or relocate a policy
  registration list. Volume::view clones the owner's private root descriptor;
  ExecutionStorage is sealed and derives both bootstrap views from retained
  owners. namespace_for_root checks consistency with the soon-removed policy
  owner. Architecture now explicitly deletes that redundant consistency check.
- D2: accept the enforcement-documentation gap. Section 4.5 now names root/mount
  confinement, private proc, non-dumpability, descriptor audit, read-only mounts,
  caps/seccomp and pipe-relayed stdio as the combined integrity boundary.
- Admission ordering: accept clarification. Section 4.6 now orders cgroup join,
  restricted leaf, probe, readiness, admission. No new channel/state required.
- E1: superseded, not suppressed. global-label-v3 contains separately captured
  anonymous-inode source and required-marker success. v2 evidence is unchanged.
- Reject broad inference that zero-capability success proves the production
  path: different idmapped mounts/seccomp and retained capabilities need actual
  regression evidence. Also distinguish skipcap's prior nonzero result from
  ordinary LSM denial semantics; source preserves it without reinterpretation.

Consensus reply is running in policy-revision-consensus. These source-grounded
fixes change the architecture review scope; no clean pass is claimed yet.

## Frozen-scope adherence passes

Scope hashes: policy-revision-scope.json. All three passes below are parent
self-reviews, not three independent reviewers. Claude consensus supplies an
additional source-grounded review, not substitute passes. No design edits
occurred between these passes. Each pass includes requirements, Rust style,
ownership/failure invariants, evidence scope and preservation.

1. Requirements-first — CLEAN. Read DESIGN trust/lifecycle against SANDBOX
   sections 4 and 10 and the architecture revision. Fixed exact-name exception,
   SELinux disabled, no registrations or policy ownership in the binary, real
   command restrictions, both bootstrap roots and fail-closed admission agree.
   Method-based anonymous-inode ownership and existing sealed step transitions
   satisfy style without a new mutable policy state machine. Cgroup joining,
   quiescence and retained cleanup owners remain explicit. v2/v3 are mechanism
   evidence only; old implementation/history is retained, not silently removed.
2. Ownership/failure-first — CLEAN. Traced Volume::view/private root cloning,
   sealed ExecutionStorage::send, RecordedDomain::launch acknowledgment,
   CommandReceiver readiness relay, EnteredRoot::restrict and descriptor audit,
   and teardown's current drain-to-policy-cleanup chain. The revision removes
   only policy-consistency/pinning phases while preserving the owned storage
   source and failure cleanup boundary. Probe FD closure precedes readiness;
   no new protocol, forgeable receipt, unsafe Rust or destructor-based durability
   guarantee is proposed. Missing-policy and dual-root tests remain required;
   no current production success is claimed. Historical source stays in Git.
3. Evidence/claims-first — CLEAN. Verified v3 policy/loader/probe/runner bytes
   match maintained fixture sources, both off controls and anonymous/overall
   success markers exist, and verdict explicitly denies production qualification.
   Reviewed captured guest scripts and native amd64 configuration: no idmapping,
   production seccomp or confidential-platform inference. Claude consensus is
   terminal exit 0 and closes D1/D2/ordering/E1 with narrowed capability/ret
   wording. Frozen document hashes rechecked unchanged. Style and ownership
   requirements still map to safe descriptor ownership and consuming existing
   states; all migration/failure tests remain implementation obligations.

M2 revision accepted at 3/3. This restores only the architecture gate. M4–M6
remain reopened at zero until replacement implementation and composed tests
pass their own three reviews. M7 stays at zero; M8 retirement remains pending.
