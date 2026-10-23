# Corrected architecture adherence gate

Scope hashes: architecture-scope.json. Baseline application code fa0d0e2 plus the
updated architecture navigation (no runtime changes). These are three self-review
passes, not independent reviews or repeated test runs. The prior failed gate and
all failed namespace-input runs remain in ../external-network/.

Pass 1 — requirements to owners: CLEAN. Reviewed the complete architecture's
module map, state/value transitions, errors, pipeline, formatting/environment,
test mapping and dedicated-VM sections against DESIGN.md and SANDBOX-DESIGN.txt.
Latest decisions take precedence: environment owns fixed label policy and enabled
networking; kernel filesystems, mapped root, cache-backed layers, direct bootstrap,
firmware pair replacement and typed transforms remain unchanged. NetworkInput
captures both enabled inputs before command user namespaces; disabled steps do
not consume them. Namespace type is enforced at trusted setns, not guessed from
procfs links. Style remains meaningful immutable owners and sealed storage/step
states; internal FD duplication is not public Clone. Cleanup and publication
remain separate. Evidence boundaries and old-source preservation are explicit;
no architecture claim depends on unperformed platform or distro validation.

Pass 2 — ownership and failure to requirements: CLEAN. Followed CapturedBuild,
CommandNetworking, CommandNetwork, sealed NetworkedStorage, volume transfer and
PrivateNamespaces. The supervisor never enters the supplied namespace. The
receiver joins with CLONE_NEWNET before changing user namespace and drops the FD;
capability reduction excludes NET_ADMIN/NET_RAW/SYS_ADMIN. Quiescence precedes
resolver release and storage finalization. External network handles are released
without deleting environment state. Failed setup retains the blocked command;
failed resolver cleanup owns its source and original deadline. No network creator
or policy-revocation state remains. Other architecture owners retain consuming
success/failure boundaries, RAII without destructor guarantees, and safe-library
interfaces. Requirements, style, tests/claims and preservation were rechecked;
actual source anchors agree with the revised design. No new finding.

Pass 3 — evidence and preservation to architecture: CLEAN. Checked maintained
input/plan/recipe, namespace, resolver, directive and publication test anchors and
the current namespace-input v4, DNS v1 and wrong-type v1 raw status/assessments.
Wrong-type entry fails before image admission; pin replacement keeps the captured
identity; resolver restoration and offline isolation are observed. The failed
procfs-type assumption is explicitly corrected, not hidden by the tests. These
observations support architecture mechanisms only; failure/authority, ingress,
IPv6, complete M4–M6 gates, distro boots and platforms remain implementation or
qualification obligations. Source-level sealed states, private handles, safe
code, independent primary/cleanup diagnostics and VM-disposal contract were
rechecked. Historical implementation and exploratory evidence remain recoverable;
local-only large artifacts are not called durable archival. No scope reduction or
architecture finding. M2 streak reaches 3; M4–M6 remain at 0.
