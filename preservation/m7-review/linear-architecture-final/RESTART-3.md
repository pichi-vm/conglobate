# M2 restarted adherence pass 3 — clean

Frozen scope: 83265df. Self-review, requirements/evidence-first order, streak 3/3.
This completes the architecture gate only, not privileged behavior qualification.

Requirements: compared the current context/lock, command, storage, firmware and
inventory contracts to their architecture owners. Git input is local committed
bytes, no redundant checkout or command context mount. Multi-architecture index
pins select the host architecture. Firmware replacement is paired; generic DTB
transformation and default applicability are independent of tool-image ancestry.
Execution uses explicit mapped identities, finite per-command timeouts, per-step
mount isolation and one retained scute per carapace stage. The environment owns
network provisioning, exact-name labeling policy and crash shutdown. No unsupported
workspace-ownership decision or pichi transport dependency was inferred.

Evidence: all eight recipe section combinations have parsing tests; planning
roles and implicit tool requirements have separate tests. Context fixtures cover
object verification, packed storage, filters, modes/links and post-capture changes.
Runtime and command lifetime claims map to maintained component tests, compile-fail
examples and explicitly limited VM results. Corrected lifecycle README matches
actual no-record assertions. Full pipeline evidence validates stage composition
and effective mappings; remaining hardware/distro/negative coverage is explicitly
unqualified. Further M3 tests may strengthen preflight without changing architecture.

Style and safety: examined the read-only values, sealed typestate interfaces,
consuming state outputs, private CompletedImage and owned failure paths against
the user's Rust preferences. Methods carry domain responsibility; shared immutable
results differ from mutable authority. No unsafe application code or new generic
workflow engine. Helper setup and kernel effects still need M4 qualification;
architecture does not claim Rust types prove confinement.

Failure ownership: finite cutoffs do not promise termination of blocked kernel
I/O. Explicit cleanup is required for success; failures retain state and diagnostics
or terminate under the disposal contract. Immutable input capture and output rehash
prevent pathname identity from substituting for consumed content. Publication
requires complete runtime cleanup and bound inventory, with no replacement.

Preservation: old code/validation and user-untracked files remain. Large local
assets still need durable archival before M8; no premature retirement or completion
claim. No findings in this pass. Three consecutive restarted reviews are clean.
