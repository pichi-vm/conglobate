# Architecture adherence attempt: findings, not a clean pass

Baseline 303f765. Reviewed normative registration-free/bootstrap/lifecycle
sections, architecture ownership table and transitions, current module/type
locations, Step cancellation/reaping/draining/finalization, receiver failure
ownership, pipeline orchestration and inventory decoding/publication boundaries.
This attempt stopped on documentation findings; it is not a complete clean review.

1. REIMPLEMENTATION's summary table still claimed M2 complete at 3/3 despite
   the later explicit reset after the normative bootstrap correction. Corrected
   to reopened, 0/3. Historical pass records remain unchanged.
2. ARCHITECTURE mapped Step to execution, artifact publication to a nonexistent
   artifact module, and listed proposed owners such as RecoveryLease,
   ImageComposition, FirmwarePair and Session types as if they were the current
   interfaces. Replaced with actual module/type ownership and scoped descriptions.
   No new abstraction or source mutation is justified merely to match old names.

Style/ownership observations: current Step uses sealed state traits and consuming
   operations; failure owners retain storage/namespace and exact cleanup phase.
   ReceiverStartup owns mount/relay/deadline; wait failure retains running mount.
   Inventory decode explicitly does not prove successful execution. These sampled
   observations do not constitute full Rust-style or security acceptance.

Evidence: targeted current positive/negative bootstrap and rootless suite exist;
   historical command-integration still depends on retired policy APIs. Its broad
   failure coverage is not current evidence. M4-M6 remain reopened. Full distro,
   architecture/confidential-platform and retirement criteria remain outstanding.

Preservation: no production code, original references or validation data removed.
   Earlier document versions remain in Git and the architecture preservation
   archive. Corrected architecture hashes captured in scope.json. Begin a new
   full three-pass sequence only after the revised scope is settled.
