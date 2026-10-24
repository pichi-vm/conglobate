# M2 architecture adherence — pass 2, clean

Scope: unchanged 8fdf264 implementation/design. Self-review. Reading order:
publication and error exits back through stage owners to acquisition. Streak 2/3.

Requirements: publication consumes privately assembled output only after runtime
close. StagedDirectory is a lower-level API, explicitly not an execution receipt.
Cache streams and produced scutes are verified while copied to private staging;
NOREPLACE protects an existing destination. Bootstrap contributes output storage,
not tool ancestry; firmware replaces the pair and transform handles the final DTB.
Formatter properties are typed data, never shell flags. Network input capture pins
one external namespace and resolver bytes; offline setup stays a separate path.
All these boundaries match DESIGN and SANDBOX-DESIGN's environment/binary split.

Failure/authority: inspected Runtime creation/close and scope failure variants,
StageFailure and CopyCleanup, ExecutionStorage transfer/finalization, preparation
failures and Teardown phases. Operations return state-specific owners or retained
failure owners, rather than pretending partial failure restored the input state.
Failed command results survive cleanup. Empty-directory barriers prevent runtime
close from accepting lingering volume/domain resources. A deadline bounds accepted
completion, not uninterruptible kernel work; the environment must dispose the VM.
No new cgroup journal, guardian, policy registration or enabled-network allocator
is required. Trusted input paths remain an environment responsibility.

Style: ExecutionStorage derives mount views from its retained Volume or both
BootstrapVolumes; callers do not substitute an unrelated raw root handle. State
traits are sealed through StorageLayout/StepStorage. Runtime phase enums hold
different typed failure resources and support bounded retry; they are not mutable
booleans purporting to prove kernel state. Immutable IdMappings and Ext4Format
are validated values. RAII is paired with explicit fallible completion; no unsafe
extraction or destructor-based success assumption is introduced.

Evidence/schema: checked current inventory construction against final manifest,
produced scute suffix and effective formatter/profile identities. Existing tests
exercise tampered inventory, failed publication and metadata validation. The
native full pipeline is positive integration evidence; CI currently covers only
six listed native lifecycle cases. The architecture explicitly leaves missing
negative cases and platform/distro qualification for later gates. No claim of
attestation from the unauthenticated CBOR inventory is made.

Preservation: reference snapshots and failures remain present. No large local
artifact is reclassified durable, no old validation is removed, and untracked
user review streams are untouched. No architecture change is needed from this
pass. This does not approve unqualified implementation failure paths.
