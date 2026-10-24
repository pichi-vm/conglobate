# M2 architecture adherence — pass 1, clean

Scope: 8fdf264 architecture, normative design and current ownership interfaces.
Self-review, not an independent review. Reading order: inputs through publication.
The former UID/GID decision is resolved by explicit environment inputs. Earlier
reviews remain historical; this starts a new streak at 1/3.

Requirements: BuildRequest captures explicit paths and paired validated ranges;
CapturedContext reads verified committed bytes without executing checkout hooks;
BuildPlan selects ordered stages and locked roles. ResolvedBuild verifies local
inputs and rejects missing execution roots. Pipeline sequences bootstrap,
carapace and firmware; StageOutput separates ancestry from tool inputs and uses
sealed roots without unnecessary filesystem activation. Carapace shares one
writable snapshot across directives. All stages use captured ID mappings.
Network provisioning, label-policy installation and crash disposal remain external.

Style and states: immutable private-field descriptions precede authority. Methods
on Pipeline and stage owners express the sequence. Sealed StepState, VolumeState
and storage traits prevent external implementations; transitions move owning
payloads. Arc<SealedScute> shares immutable results, not command authority.
Optional StageOutput components describe valid output combinations. No generic
workflow scheduler or persistent state journal was introduced. Cargo forbids
unsafe application code; safe mount APIs remain pinned and documented.

Failure ownership: Runtime::scoped cannot return runtime-borrowing values and
rejects successful operation data if close fails. StageFailure retains indexed
failure/storage ownership. Teardown carries the original deadline through retry,
and never treats successful cancellation as successful command execution. Storage
and mapped namespace retention on unfinished Step deliberately outlives best-effort
Drop; VM disposal is the terminal contract. ControlMountNamespace verifies the
acquisition namespace rather than PID 1. CompletedImage is private to pipeline;
publication follows completed assembly and bound inventory, using NOREPLACE.

Schemas and evidence: inventory version three records effective maps, actual
selected input identities, output components and formatter receipt; network
configuration remains excluded. Decode proves representation only. Recent full
pipeline evidence covers higher IDs across sealing/remount, stage-level scute
count and DTB binding. Rootless checks cover invalid ranges and absent execution
maps. Architecture tables explicitly identify missing failure, distro and platform
qualification rather than treating these results as blanket passes.

Preservation/scope: no reference or user-untracked data removed. Small evidence
and source snapshots are committed; large assets remain local and block M8 durable
retirement. RecoveryDirectory's cooperative lock/name is a simplification candidate,
not cross-build recovery or a security boundary. Current namespace ownership
checks must survive any later simplification. No architecture finding requiring
a change was identified. M4–M7 behavior still requires its own qualification.
