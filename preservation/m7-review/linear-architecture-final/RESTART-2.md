# M2 restarted adherence pass 2 — clean

Frozen scope: 83265df. Self-review, failure-first order, streak 2/3.

Requirements and ownership: revisited the external environment split and following
failure chain: formatter staging -> restricted labeling admission -> command
observation -> teardown -> storage finalization -> RuntimeScopeFailure -> terminal
BuildError. Environment inputs never authorize recipe control of formatter flags,
control paths or kernel policy. Runtime cleanup failure blocks StageOutput from
escaping to assembly. Pipeline error conversion preserves operation and cleanup
causes and requires VM disposal when retained resources cannot be recovered.

Style/safety: CapturedFormatter owns captured ordinary bytes; FormatterRuntime
owns its detached read-only root and backing. Cleanup failure retains backing,
not a success receipt. AssignedRoots is trusted layout data; LabelProbe uses
anonymous owned inodes and closes before readiness. CommandResult consumes cleanup
transitions while retaining the original result, and ObservedCommand fixes the
teardown cutoff once. Those methods model real invariants; no new mutable phase
flags or unsafe application calls are needed. Sealed state traits and immutable
plan/value types remain consistent with the architecture table.

Evidence: test/document boundaries were rechecked against negative-case descriptions
and fixture entrypoints. No command-record creation/removal claim remains in the
maintained README. Tests for missing policy and failed second-root labeling are
explicitly narrower than platform isolation proof. Formatter and lifecycle fault
cases remain separate from full pipeline success. CBOR is an inventory, not
attestation; actual bound output hashes and effective mappings are recorded.

Preservation: previous findings, source snapshots and failed runs remain; local
large artifacts still need durable M8 archival. No user file or legacy evidence
was removed. No additional architecture finding. Implementation qualification
and full adherence for M3–M8 remain required.
