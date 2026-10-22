# Milestone 2 adherence gate

Scope: ARCHITECTURE.md, applicable DESIGN.md schema refinements, and the safe
Rust architecture probe. This is architecture acceptance only; dependency gaps
and kernel behavior still require their implementation milestone gates.

## Initial review — FINDINGS, streak reset to zero

1. Filesystem capacity was not distinguished from the library's fixed device
   geometry. Corrected using pinned carapace source and explicit formatter block
   count; schema now rejects unsupported/alignment/overflow cases.
2. A quiescent process domain could be confused with successful execution.
   Corrected: outcome is retained; only success plus clean teardown produces a
   Completed step and receipt. Cleanup cannot rehabilitate a failed command.
3. Scoped-policy identity lifetime after concurrent supervisor/guardian death
   was underspecified. Added persistent root-only kernel-held recovery anchors,
   containment-before-admission and ordered recovery. Qualification remains M4.
4. Cached safe syscall wrappers do not expose mount_setattr. Recorded as an
   explicit dependency gap, with no application unsafe workaround allowed.

Fixes applied before the clean sequence. No implementation completion claimed.

## Pass 1 — normative requirements to ownership boundaries — CLEAN

Reviewed DESIGN.md sections 2–12 and SANDBOX-DESIGN.txt sections 1–12 against
architecture module and evidence tables. Explicit recipe/lock, stage omission,
from/inheritance, bootstrap direct layer and dual roots, firmware pair extraction,
DTB defaults/CARAPACE, cache-backed reads, tmpfs, no-context command execution,
LSM-scoped authority, network/DNS policy and final publication each have an owner
and planned test boundary. Filesystem syntax and inventory formatter receipt
fields are explicit. Platform/recipe behavior is not replaced by prototype
convenience. Preservation/retirement and unavailable qualification remain gated.
No findings. Clean streak: 1.

## Pass 2 — type ownership, style and failure direction — CLEAN

Reviewed state transitions from final publication backward to raw inputs. Private
validated values are separate from deserializable documents; state families are
sealed; authority owners are not Clone; scoped views prevent consuming unmount
while borrowed. RAII payloads permit safe consuming moves. Quiescence and command
success are distinct. Transition failures preserve known state or retain a
RecoveryLease; Drop does not certify completion. Runtime input alternatives use
enums rather than state flags. Free functions are confined to test harness entry
points in the probe; its boolean models injected transition success only, not a
proposed mutable production state. Prototype tests verify ownership recovery and
compiler rejection of illegal state/borrow operations. No unsafe application
code is introduced. No findings. Clean streak: 2.

## Pass 3 — external state, recovery and evidence claims — CLEAN

Reviewed source/API evidence and death paths separately from Rust ownership.
Pinned carapace geometry and empty-chain rejection explain explicit devmap zero
activation; no image conversion reappears. Cache mutability must remain checked
through verified reads. Policy authorizes an explicit filesystem set, stays
immutable during commands, and cannot be removed by an unrelated owner.
Persistent namespace/filesystem anchors prevent the design from relying on stale
numeric IDs after simultaneous controller death. Unknown ownership is quarantined;
recovery and publication are distinct. Safe mount_setattr integration is an honest
M4 dependency gap, not a claim that cached rustix supports it. libbpf-rs is a
candidate, not a completed license/security qualification. No VM, distro or
hardware coverage is inferred from the compiler probe. Architecture includes
falsifiable M4/M5/M7 gates and reopens if those fail. No findings. Clean streak: 3.

All three passes are self-reviews of the same fixed architecture scope, not
independent agents. Ledger recording does not change reviewed scope. Milestone 2
is accepted; milestone 3 foundation work may begin. The new runtime is not approved
or implemented by this gate.
