# Architecture re-review — pending identity-range decision

Scope: 23f7d83. Input-to-output reading order after correcting the first review's
documentation findings. This is a self-review; no clean pass is recorded while
the supported image-ID contract remains unresolved.

## Requirements and data ownership

BuildPlan is immutable after construction. It records stage order, locked input
roles and network-enabled step indexes. Bootstrap tools always require a root;
a carapace base requires one only for directives or implicit firmware tooling;
explicit firmware tools require their own root. CapturedBuild::resolve verifies
all selected content and rejects missing execution roots before stage setup.
This agrees with inheritance and firmware-only pass-through semantics.

StageOutput derives CARAPACE from produced scutes or the selected verified base,
not the firmware tool image. No output mapping is needed for that identity.
Bootstrap returns only its sealed output/receipt; the writable tools are discarded.
Carapace shares one snapshot across directives and seals after the executor
returns clean unmounted storage. Firmware returns captured files after command
and tool cleanup. Runtime cleanup precedes shared transformation and packaging.

The final manifest and inventory are bound before CompletedImage can publish.
StagedDirectory::publish uses NOREPLACE and has no fallible operation after the
rename, avoiding an error result that hides successful publication. The private
parent FD controls destination resolution. Source staging/storage must remain
trusted; raw staging APIs alone are not successful-build receipts.

## Style and state invariants

Checked values have private fields; decoded wire data does not deserialize into
runtime authority. Volume and command state traits are sealed and state payloads
own the resources required by each transition. Process/cgroup identity is retained
through descendant drain; a Rust borrow alone does not represent a live process.
The synchronous pipeline is expressed with methods and consuming stage outputs,
not a generic scheduler or persisted transition graph. Optional fields in
StageOutput describe legitimately independent output components, not pending
process/mount state. Root ranges and namespaces use safe library interfaces;
application crates forbid unsafe Rust.

## Failure, schemas and evidence

Inspected captured-input validation, trusted formatter profile, inventory schema
and atomic publication alongside stage error propagation. Formatter properties
remain typed capacity/label data; generated effective arguments/profile hashes
are inventoried. Version-two inventory defines byte identities, ordered input
roles and output scutes, command network permissions, and omitted optional roots.
Resolver/network configuration remains excluded. CBOR representation checks are
not authenticity claims. Context capture and local cache errors do not fetch.

Current positive evidence is 0556f30's rootless suite and full CLI pipeline.
Negative tests cover malformed plans/inputs, output replacement and component
integrity; historical lifecycle failures are not silently relabeled current.
Unqualified failures/platforms/distro boots remain M4–M7 work. No preservation
assets or user changes were removed. Large external artifacts remain an explicit
M8 dependency.

## Decision needed before a clean architecture gate

SANDBOX-DESIGN 4.2 explicitly says the prototype's 65536-ID mapping is not a
universal OS-image requirement. All three production stage setups currently use
IdRange::new(100000, 65536). IdRange rejects zero/overflowing parent ranges, and
COPY checks supplied IDs, but the initial supported range/allocation contract
has not been chosen. Asked the user whether to keep/document this initial limit
or allow an environment-supplied range. Do not infer approval from no answer.
The existing workspace ownership remains unchanged; no new input was adopted.

No additional executable validation was launched during this review. CI's Rust
job at d5e77e6 completed successfully; dependency and guest jobs were still live
at inspection. This record is not a substitute for the three clean passes.
