# External-network architecture gate

Scope: DESIGN.md, SANDBOX-DESIGN.txt, ARCHITECTURE.md, current ownership and
handoff source anchors. User decision: externalize enabled networking. This is
architecture acceptance only; implementation and VM qualification remain pending.

Initial review: NEEDS WORK. The proposal added runtime namespace-owner ioctl
introspection without demonstrating a need or a safe library interface. The
actual invariant is that command user namespaces are created after capture and
cannot own the selected external namespace. Existing command capabilities omit
NET_ADMIN, NET_RAW and SYS_ADMIN. Corrected the contract: initial ownership is
provided by the environment, type and distinct identity are checked by conglobate.
This resets the architecture streak before the passes below.

Pass 1 — requirements-first: CLEAN. CLI namespace and resolver inputs are required
for enabled plans; offline commands retain fresh namespaces. Namespace sharing is
explicit and confined to sequential steps of one build. Environment owns NAT,
all reachable destinations, ingress policy and lifetime; no systemd dependency.
Style/ownership: immutable captured aggregate, private descriptors, consuming
per-step cleanup, no new mutable policy state or public Clone. Errors preserve
resolver cleanup ownership and existing command failure rejection. Tests required
are listed in the execution batch, not claimed to pass. Old sources/evidence are
preserved at e249a90. Other architecture areas remain unchanged.

Pass 2 — ownership-first: CLEAN. Traced CapturedBuild -> coordinators ->
CommandNetworking -> CommandNetwork -> Supervision::send_network_volume_until ->
VolumeInput -> PrivateNamespaces. Owned/duplicated handles survive the step;
receiver joins before user-namespace entry and drops the FD. Descendant drain
precedes resolver cleanup. External network release does not remove interfaces or
policy; disabled path still unshares NEWNET. No need for creator, allocation or
NAT recovery owners. Sealed storage traits and consuming Step states remain.
Checked requirements, safe-code style, errors, negative-test mapping and reference
preservation; no additional architectural finding. This traces the proposed
replacement against existing boundaries, not implemented behavior.

Pass 3 — failure/evidence-first: CLEAN. Missing/wrong-type/current namespace and
path replacement need maintained negative tests. Enabled/offline, DNS restoration,
command authority and unchanged external networking after failure need disposable
VM coverage. Input capture precedes command creation; setns errors reject setup
without admitting image code. Resolver failures retain existing recovery authority.
No receipt certifies NAT configuration; trusted environment contract is explicit.
No new unsafe wrapper, process, publication path or forgotten destructor guarantee.
Rechecked requirements and style against unchanged storage/bootstrap/firmware/
inventory boundaries. Full distro/platform qualification and durable archival
remain outstanding. All three passes are self-review, not independent review.

## Reopened by implementation evidence

The three passes above are historical. Input VM v3 disproved the assumed procfs
FD-link type check for bind-mounted namespaces. Runtime capture now checks nsfs
and distinct identity, with explicit setns(CLONE_NEWNET) enforcing type at trusted
entry before image admission. M2 resets to 0/3 pending final batch review. v4
proves pinned identity survives path replacement and descriptor release leaves
the environment namespace intact; it does not exercise command admission.
