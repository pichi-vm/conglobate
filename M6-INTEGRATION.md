# M6 pipeline and publication

M5 is accepted on preservation/m5-review/scope-v2.json with three clean
self-review passes. M6 is active with zero passes. DESIGN.md and
SANDBOX-DESIGN.txt remain authoritative; this is an integration order, not a
replacement specification or a narrower acceptance boundary.

## Starting interfaces and missing connections

CapturedBuild owns committed context, selected recipe/lock identities and the
immutable BuildPlan. ResolvedBuild owns local Cache and CheckedImage inputs;
resolution verifies references/chain and rejects missing execution carapaces.
These are implemented gates, not a runnable CLI. BuildRequest parses explicit
context-relative recipe/lock paths but the crate has no executable entrypoint.

M4/M5 supply actual preparation, execution receipts, finalization and direct
sealing for one or two volumes. Their integration fixture currently supplies
orchestration and OCI metadata. Production COPY has pattern/mode descriptions
only; transforms have parsed operations/templates only. Network-enabled steps
are identified in the plan and require a resolver argument, but enabled network
setup is not connected. No production inventory/publication coordinator exists.

## Integrated implementation order

1. Connect resolved inputs to an owned output-image assembly and production OCI
   publication. Start with the valid pass-through recipe, preserving every base
   component and selecting only the host manifest. Keep execution tool inputs
   separate from output ancestry. Capture/reverify actual blob bytes used for
   publication; do not trust earlier verification of a mutable cache pathname.
   Include canonical empty config and every referenced blob. Generate deterministic
   version-two inventory from actual identities and receipts; final validation
   cross-checks it against the final manifest. Stage under a temporary sibling and
   atomically rename without replacement. Exercise through a real CLI invocation
   on a committed recipe/lock and verify the emitted layout independently.
2. Connect the same coordinator to M5 bootstrap and single-volume carapace
   execution, then confined COPY. Preserve per-directive order, per-run remounts,
   finite timeouts and original cleanup deadlines. Empty derive/pass-through must
   not invent a layer. Use sealed output directly as later-stage origin; do not
   publish intermediate artifacts or convert a filesystem image. Every error
   retains required cleanup authority and prevents final publication.
3. Add firmware execution/extraction and DTB transformation to this same output
   assembly. Firmware tools may differ from the output base and never become
   ancestry. Extract both regular PMI/DTB outputs through confined handles only
   after writers stop, validate them, and replace the pair together. Apply safe
   DTB property edits and built-in corium behavior to the final DTB. Preserve
   unchanged properties, reservation entries and header semantics. Expose the
   output root as CARAPACE, not the alternate firmware tool root. Record actual
   effective transform identity and skip status.
4. Wire enabled networking through owned setup/teardown into the existing command
   lifecycle. Disabled stays the default with a separate network namespace.
   Enabled gets outbound NAT/replies, all reachable destinations and explicit
   resolver bytes at a root-confined destination; no resolver inventory entry or
   persistent image injection. Setup/teardown have fixed internal deadlines.
   Use benign disposable-VM connectivity/DNS/cleanup fixtures only.
5. Close the complete stage/output matrix and failure regressions, including
   standalone CLI helper dispatch, no downloads, absence versus explicit empty
   transform, firmware-only output, inherited unknown components, cache mutation,
   output collision, interrupted staging, missing artifacts, cleanup failure and
   no publication after failed commands. Verify OCI interoperability with ORAS.
   Then perform the three complete adherence passes on a frozen scope.

## Ownership requirements

The coordinator owns immutable request/resolved inputs separately from active
resource sessions. Output assembly carries content provenance without ambient
path authority. Execution receipts, clean storage finalization, sealed layers
and publishable output remain distinct types; no public constructor converts a
failed command into publishable state. Use sealed marker states where phases
change available operations, consuming conversions, RAII retention and explicit
fallible cleanup. Avoid a mutable global build-state bag and duplicate single/
bootstrap execution state machines. Safe libraries own wire parsing.

All listed work is required before M6 acceptance. Package-manager builds and
boots for Fedora/Debian/Alpine/NixOS, additional architecture/confidential hardware
and full repository qualification remain M7. Preservation retirement remains M8.

## Output storage transaction implemented

StagedDirectory owns a private temporary sibling anchored to a parent descriptor.
It copies cache blobs through Cache::copy_verified, checking the exact written
stream and only installing a digest-named blob after successful verification.
The existing verify method shares that stream validation through a sink. Failed
copies discard their private temporary file. Captured/produced small bytes can
be stored by digest; metadata writes have fixed filenames. Publication uses
renameat2 NOREPLACE and returns the staging owner on failure. It performs no
fallible work after a successful rename, avoiding ambiguous publication errors.

This is a low-level filesystem transaction, explicitly not a build-success or
validated-inventory receipt. The coordinator/assembler still must validate the
manifest, inventory and complete referenced blob set before calling it. No CLI
build or complete OCI artifact is claimed yet.

Three output tests pass (late collision preservation, cache mutation rejection,
and parent rename with exact output bytes); all 12 cache tests pass including
writer failure propagation. Strict all-target Clippy passes. Source/log hashes
are in preservation/m6/output-storage. Tests use ordinary temporary directories,
no host mounts or policy loading. M6 remains at zero adherence passes.

## Inherited artifact assembly

AssembledImage::inherit creates fresh staging and copies the selected manifest's
configuration and every layer through verified streams. It preserves exact
manifest JSON bytes, including fields the shared typed view does not expose.
Canonical inline empty configuration is materialized as a blob. The output index
selects only the host manifest. Reopening staging through Cache verifies the
manifest, all referenced bytes and the carapace chain before returning an
immutable assembled owner. No mutable staging handle or publication method
escapes this owner; inventory validation/publication remains to connect.

Two new cases pass: both real and inline-empty configuration remain self-contained
with PMI/DTB after deleting the source cache, including exact manifest extensions
and no unreferenced index blob; a missing reference retains private staging for
explicit cleanup and never creates the output destination. The firmware bytes
are benign opaque fixtures: these tests prove transfer/content identity, not PMI
or DTB semantic validation. All five output tests and strict all-target Clippy
pass. Initial constructor-refactor compile failure and corrected logs/source are
preserved under preservation/m6/inherited-assembly. M6 remains zero clean passes.

## Version-two inventory wire implementation

Inventory is an immutable validated document with its exact canonical CBOR bytes
and digest. Private typed records include single recipe/lock paths, formatter
receipts, pipeline network identifiers and transform skip status. Ciborium and
serde_bytes provide safe serialization; a consuming value normalizer orders map
keys by encoded bytes and rejects duplicate keys, tags and floats. Strict decode
checks both canonical wire bytes and typed-schema roundtrip, rejecting unknown
fields, unsupported versions, nonminimal/indefinite encoding, trailing data,
optional nulls and alternative representations. Enum/byte-array types and
cross-field validation constrain roles, identities, paths, roots, output pairs,
formatter geometry/runtime order and network-step order.

This validates inventory representation, not authenticity or actual execution.
The private record constructor is not yet connected to completed stage receipts
or AssembledImage. Manifest/inventory cross-checks and the CLI remain next.
No attestation or resolver measurement was added. Five inventory tests pass,
including full formatter/network schema and negative wire corpus; all 66 library
tests and strict all-target Clippy pass. The command fixture compiles with its
updated dependency lock. Logs/source hashes are in preservation/m6/inventory-wire.
No new VM run was needed for this unprivileged serialization change. M6 zero passes.

## Production pass-through CLI connected

The binary dispatches the existing trusted helper protocol before ordinary CLI
parsing. Pipeline captures the requested commit, resolves the local cache,
validates a currently implemented pass-through recipe, materializes the private
read-only context on its owned tmpfs, assembles the image, constructs inventory
from actual captured/resolved/output identities, and closes the private context
before publication. Initial implemented transformations are authored empty with
an inherited DTB, or implicit default skipped because there is no DTB. All other
stage/directive/transform paths fail explicitly before publication; they remain
required M6 work, not optional supported-subset behavior.

A private PassThrough proof gates the no-execution path. A private CompletedImage
pairs the assembled image with its checked inventory; decoded inventory cannot
construct it. The inventory hashes /proc/self/exe, committed recipe/lock bytes,
Git identities and supplied image indices/manifests, and records output scutes in
verified chain order. The manifest/root/firmware/scute identities are checked
against output before writing inventory and atomic publication. Authored transform
identity binds exact recipe bytes; built-in identity binds the exact sequence.
DESIGN.md now makes this wire identity choice explicit.

The disposable-VM --pipeline mode invokes the production CLI, checks inventory
with a separate minicbor field reader plus the strict decoder, validates all
layout digests and exact referenced-blob membership, removes the source cache,
and verifies output again. It also checks dirty-worktree independence, destination
collision, unsupported commands and missing cache without publication. It does
not execute image-authored commands or qualify distro boots. v187 passed; v188
adds export of the exact final index, manifest and CBOR bytes for preservation.

v188 passed and exported exact index/manifest/inventory bytes. The host verified
the exported index's manifest digest/size. Both runs' source, context/cache fixture,
logs, verdicts and external kernel/initramfs/binary hashes are preserved under
validation/rewrite-command-integration/evidence/v187 and v188; the earlier fixture
source reconstruction is explicitly identified. No full normal command-suite,
ORAS, distro/boot or confidential-platform result is inferred from these runs.

## DTB executor

TransformSelection::apply executes the validated sequence through shared devtree
and returns immutable output plus the observed implicit-default skip state.
Property types, missing targets, conditional skipping, all-match regex behavior,
variable failures, unchanged-byte preservation and header/reservation preservation
are covered by tests. The default definition is shared with inventory hashing.
This is not yet called by the coordinator. Next replace the staged DTB descriptor
while preserving unrelated manifest fields, bind inventory to the execution
receipt, and exercise success/failure through the real CLI in the disposable VM.

## DTB publication integration

InheritedBuild now executes transforms with verified DTB/root inputs, constructs
an updated self-contained manifest, and records observed transform disposition.
No-op edits preserve exact manifest bytes; changed DTB descriptors retain their
unrelated fields but discard obsolete inline content/location hints. Output
assembly revalidates all references before inventory binding/publication.

v190 passes real-CLI default and authored edits plus missing-DTB/property failures
without publication. v189's diagnostic-expectation failure is preserved. Next
connect stage execution and COPY, firmware pair extraction, and enabled networking;
this successful inherited-image workflow is not full recipe execution.

## COPY sources

CopySources selects immutable captured Git entries without ambient filesystem
reads. Explicit selected symlinks resolve within that tree; descendants of a
selected directory retain their link entries for the writer. Wildcard walking
uses the existing Go-style matcher and does not descend through matched symlinks.
The next boundary is confined destination writing on an inactive mounted volume,
including image account lookup, owner/mode application and directory merge rules.
The selector is not yet connected to CLI directive execution.

## COPY permissions

CopyMode now evaluates octal/symbolic descriptions. An executed comparison against
BuildKit's mode library covers 3,514,368 outputs and revealed behaviors the legacy
evaluator missed (original-mode X and ordered permission copies). Digest vectors
are maintained Rust tests; generator/reference provenance remains preserved.
Destination writing and owner/group lookup still need implementation and VM
validation before COPY is connected to the coordinator.

## Mounted-volume COPY writer

Volume<Mounted>::copy exclusively borrows storage and writes captured entries
through pinned descriptor-relative paths. Image symlinks are confined; account
reads reject special files; replacements avoid following nested links/hardlinks.
Ownership precedes reference-validated permissions. v191 demonstrates committed
bytes and UID/GID/mode on ext4 within the disposable VM; normal lifecycle tests
still pass. The coordinator must now arrange these calls between command steps,
route bootstrap tool/output destinations, and bound trusted work. COPY is not yet
available as an end-to-end CLI build directive.

## Bootstrap COPY routing

BootstrapVolumes<Mounted>::copy presents tool / and output /mnt through pinned
handles, including cross-root symlinks and /mnt/.. behavior. v193 validates the
production APIs in a command/COPY/command sequence with remounts, persisted
contents/ownership/mode and no context exposure. Its full normal VM suite passes.
The earlier missing-ln fixture failure is retained as v192. Next use this sequence
in the actual pipeline coordinator and finish trusted-work deadline handling;
fixture orchestration is not CLI completion.

## Production directive sequencing

CommandSandbox::directives now owns ordered RUN/COPY execution for both storage
layouts, returning an unforgeable completed-directives receipt or owning failure.
RUN uses its configured timeout and explicit CARAPACE; COPY shares fixed setup
and teardown deadlines and validates flushed snapshots. Cleanup retries cannot
promote a failed COPY to successful execution. Network-enabled recipes fail
preflight until networking is integrated.

v194 uses this runner, rather than hand-written fixture sequencing, for the mixed
bootstrap recipe. The normal VM suite and host checks pass; a fixture-only Clippy
correction is preserved separately. Next connect CLI image construction, seals,
output provenance/inventory, and dedicated runner failure cases.

## Produced-scute assembly

AssembledImage::extend copies inherited references, streams a sealed scute into
verified private blob storage, updates the manifest root, and revalidates the
complete output chain. The constructor is an artifact proof, not a successful
recipe receipt. v195 exercises real two-layer assembly and wrong-parent rejection;
normal VM lifecycle tests remain green. Next bind stage execution/sealing to this
constructor and final DTB/inventory publication in the coordinator.

## Runtime ownership integration

The production storage::Runtime now owns a private attachment of a subdirectory
of the existing Workspace tmpfs, its bpffs mount, a cgroup2 view with an exclusive
build subtree, and the recovery lease. RuntimePaths borrows this owner and gives
CommandSandbox its protected paths. Helpers receive canonical deployment paths;
supervisor cleanup uses its pinned parent descriptor. This avoids passing
/proc/self/fd paths whose meaning changes when helper descriptors are closed.

Runtime close consumes the owner, checks empty recovery/volume directories and
policy pins, removes the private cgroup subtree, and performs non-lazy unmounts.
Cleanup errors retain the remaining mounts and original fixed deadline. Dropping
unfinished ownership retains control state until dedicated VM disposal. Partial
acquisition only owns exclusively-created directories; no second tmpfs or
recursive removal of command state is introduced.

The disposable-VM command fixture now uses this runtime for its two-command
sandbox sequence and command preparation failure paths. Full CLI integration is
still required: select an executable build, acquire Runtime under the deployment
parent, execute directives on verified snapshots, preserve failures through
cleanup, seal accepted storage, bind produced origins in inventory, and publish
only after explicit runtime/context cleanup. Acquisition fault-injection coverage
and full M6 review remain required; this is not a milestone adherence pass.

## Carapace directives connected to CLI

Pipeline now executes carapace RUN/COPY directives through CarapaceBuild and the
owned runtime. Each directive creates its own sealed scute and extends the active
read-only chain for the next directive without copying inherited cache-backed
layers. Produced data/hash files are immutable shared owners retained through
mapping teardown and final streaming output assembly. The final root feeds DTB
transformation, and inventory distinguishes inherited and produced scutes.
Publication follows explicit runtime and context cleanup. v200 demonstrates a
real committed three-directive CLI build, output remount, and nonzero-command
rejection; v201 preserves the earlier pass-through and transform coverage.

Bootstrap and firmware coordinators, enabled networking, the complete matrix and
failure coverage, ORAS verification, and M6 adherence review remain outstanding.

## Firmware permission for DTB edits

Publication now enforces DESIGN.md section 10 through DtbContract. A changed DTB
requires every known PMI target to accept one external measured DTB with sufficient
Zero-section capacity, and rejects pre-signed SEV identities. Opaque or otherwise
unsupported firmware can pass through when the DTB is unchanged. v202 covers the
CLI boundary and host tests cover target/schema/capacity/ambiguity rejection.
The earlier opaque-success transform fixtures were incorrect for this requirement;
they are retained in historical evidence and replaced in maintained validation.
Firmware execution/extraction must apply this same check to its resulting PMI/DTB
pair before publication; that coordinator remains pending.

## Firmware-only and inherited-base replacement CLI paths

FirmwareBuild executes directives on a disposable tool snapshot, captures both
regular outputs using root-confined handles, tears down storage, and finalizes
the DTB against the new PMI contract. Publication emits either a fresh pair-only
image or the unchanged inherited output base with its firmware pair replaced.
The tool image is never implicit output ancestry. v203 checks both paths, root
and inventory semantics, declared-output symlinks and missing-file rejection.

Composition with newly produced carapace layers is now implemented as described
below. Bootstrap coordination, enabled networking and the remaining qualification/
gate work are still open; the full stage matrix has not been accepted.

## Derived carapace followed by firmware

DerivedCarapace transfers the successful sealed layers, active read-only chain
and reaper into firmware execution. Implicit tools use this chain; explicit tools
use their pinned input chain while CARAPACE still denotes the output root. Final
assembly retains only the selected base and produced carapace layers, replaces
the firmware pair, and records produced origins. No intermediate image is
published and no inherited DTB transform precedes a replacement firmware stage.
v204 verifies both tool-selection modes, root binding, remounted output contents,
disposable firmware changes and failure after successful carapace derivation.

### M6 formatter execution receipts

- Previous status-only turn was no implementation progress; resumed work against
  current source. Bootstrap CLI integration revealed that successful formatter
  execution had no inventory receipt transition. This gap is now closed without
  treating a planned formatter invocation as execution evidence.
- FormattedSnapshot::into_recorded_parts consumes a successful/reaped formatter
  result and returns its resource owners plus immutable FormatReceipt. The receipt
  retains exact executable/runtime identities and effective typed ext4 format.
  Its constructor is private; recipe plans and decoded inventory cannot mint it.
- Inventory::with_formatter consumes that receipt into the existing version-two
  schema, including profile/configuration digests, runtime paths/digests, arguments
  and filesystem geometry. Duplicate bootstrap formatter attachment is rejected.
  Resource cleanup, command success, sealing and publication remain separate gates.
- Host tests (86 library tests, integration suites, 52 doctests), root/fixture
  strict Clippy and formatting pass. v205 full normal disposable-VM suite passed
  in 132.84 guest seconds. The fixture compares receipt identities to the runtime
  of an actual successful format, then tests CBOR encoding/decoding in memory.
  That probe document is not published as a bootstrap CLI build. Source, executable
  identities and evidence are preserved under validation/rewrite-command-integration/evidence/v205.
- Bootstrap CLI orchestration remains unfinished. In particular the production
  CLI still needs trusted formatter runtime selection (currently supplied only
  by the validation fixture), composition with later stages, and receipt attachment
  after successful cleanup. Networking, remaining matrix/fault tests, ORAS and
  three full M6 adherence passes remain outstanding. M6 streak remains zero.

### M6 standalone bootstrap CLI

- Previous goal turn progressed in 50ec86b (formatter execution receipts).
  Connected standalone bootstrap to the actual CLI. The trusted environment now
  supplies --formatter-runtime PATH: strict YAML root/executable/files selection,
  separate from authored recipe properties. Relative roots resolve against that
  manifest; captured executable/runtime bytes and builder-owned configuration are
  retained/measured. Missing runtime files fail; no ambient fallback or downloads.
  DESIGN.md documents this interface. Unit coverage rejects duplicate/unknown
  fields, duplicate runtime paths, escaping paths and missing files.
- BootstrapBuild creates a zero-backed writable snapshot, executes the captured
  typed formatter, closes its runtime, activates the verified cache-backed tools
  with a disposable writable overlay, and executes bootstrap RUN/COPY through the
  existing two-volume directive runner. Tool changes are discarded; output is
  sealed directly as a zero-parent scute. No intermediate filesystem image/import.
- Command failures use retained pair cleanup, formatter failures reclaim reaped
  resources (or retain unfinished ownership), and runtime cleanup is required
  before returning publishable state. Default DTB handling and the actual
  formatter receipt enter the inventory; private context cleanup still precedes
  atomic output publication. Tool-carapace components never become ancestry.
- v206 full normal disposable-VM suite passes (148.56 guest seconds). New CLI
  cases cover fresh output, committed-context COPY despite dirty worktree,
  command failure, missing --formatter-runtime, and an incomplete loader runtime.
  The output is reopened through the verified storage chain: one produced scute,
  requested files present, tool /bin and disposable tool writes absent, no inherited
  firmware, formatter label/capacity recorded. Failures publish no directory and
  leave no runtime attachment. Source/evidence/executable hashes archived under
  validation/rewrite-command-integration/evidence/v206. Host tests (87 library,
  integration suites, 52 doctests), strict root/fixture Clippy and formatting pass.
  Host observation timeout increased to 180 seconds for the additional CLI cases;
  production setup/command/teardown deadlines are unchanged.
- This enables standalone bootstrap only (including an empty carapace stage).
  Bootstrap followed by retained derivation or firmware remains explicitly
  rejected and required next. Enabled NAT/DNS, remaining matrix/variable/fault
  coverage, ORAS and three full adherence passes remain outstanding. M6 stays
  active at zero clean passes; no full distro/platform qualification is claimed.

### M6 bootstrap followed by retained carapace layers

- Previous goal turn progressed in 83486fe. Bootstrap output can now feed retained
  carapace directives directly. ReadOnlyChain::from_root accepts only a sealed
  zero-parent root and activates its RAM-backed scute without an intermediate
  cache image/import. Existing cached-base execution and this fresh-root path share
  DerivedCarapace::derive, consuming active-chain/scute/reaper ownership and sealing
  one layer per directive. Bootstrap's original tool components remain excluded.
- Fresh output assembly accepts the complete ordered produced chain, checks every
  parent starting at zero, streams every scute, sets the final root annotation,
  and verifies the self-contained staged layout. Inventory binds all layers as
  produced and retains the single actual formatter execution receipt. No output
  publication occurs until all commands and cleanup succeed.
- v207/v208 exposed a synthetic-root limitation: the fixture installed a shell
  and libraries but omitted /dev, /proc and /sys mountpoint directories required
  by the existing command runtime. Failure surfaced as registration peer loss,
  truncated helper diagnostics and retained runtime cleanup. Preserved both
  failures before changes. Supervisor preflight now validates the same confined
  runtime targets before policy/command launch, giving an explicit missing-target
  error and normal rollback. Helper diagnostics render before writing; this does
  not promise delivery after cancellation and adds no synchronization sleep.
- The valid fixture explicitly supplies its root skeleton through measured COPY.
  A separate missing-target recipe verifies failure names /dev, publishes nothing,
  and leaves no runtime attachment. Automatic creation/restoration of absent
  runtime mountpoints is not implemented: current command roots must supply /dev,
  /proc and /sys (or valid confined symlink targets). This limitation remains visible
  for subsequent distro qualification; no arbitrary-root compatibility is claimed.
- v209 reached all expected CLI outcomes but failed an existing firmware inventory
  assertion because a broad fixture edit accidentally changed its conditional.
  Preserved the run and restored the original firmware assertion without changing
  production behavior. v210 full normal VM suite passes (176.62 guest seconds),
  including bootstrap -> COPY/RUN/COPY, four produced scutes, committed copy bytes,
  retained data, no disposable tool changes, real output-chain reopening, failed
  later RUN with no output, and rejection of a nonzero-parent root constructor.
- Evidence/source/executable identities for v207-v210 are preserved under
  validation/rewrite-command-integration/evidence. Host tests (87 library tests,
  integration suites, 52 doctests), strict root/fixture Clippy and formatting pass.
  Host suite observation timeout is now 210 seconds for the expanded cases;
  production execution/setup/teardown deadlines are unchanged.
- Firmware after bootstrap remains explicitly unsupported and is the next
  composition path. Enabled networking, remaining variable/matrix/fault coverage,
  ORAS, three full M6 adherence passes and all later milestones remain required.
  M6 stays active at zero clean passes; these checks are not a milestone review.

### M6 firmware after bootstrap

- Previous goal turn progressed in 2f8c9aa. Removed the bootstrap/firmware
  composition rejection. FirmwareTools now owns the selected read-only tool chain,
  separate output-root identity and transferred child reaper. Both cached-base
  FirmwareBuild and BootstrapBuild consume the same firmware command/extraction
  implementation; no second executor or intermediate cache publication is added.
- Bootstrap with retained directives transfers its active derived chain to
  implicit firmware tools. Explicit firmware.from closes that chain and activates
  the selected cached tools while retaining immutable output scutes. Without
  retained directives, implicit firmware activates the fresh sealed root directly;
  explicit tools do not require a runnable shell in the retained bootstrap output.
  CARAPACE always denotes the output root, not the selected tool root.
- Fresh firmware output assembly now accepts the complete zero-parent produced
  chain, checks each parent, supplies canonical carapace metadata, and appends the
  captured PMI/DTB pair. No bootstrap tool components or disposable firmware writes
  become output ancestry. Final DTB transformation occurs after firmware capture.
  The original formatter receipt and all produced scute origins reach inventory;
  cleanup remains required before publication.
- v211 full normal disposable-VM suite passes (233.33 guest seconds). New CLI cases
  cover implicit/explicit tools with/without retained carapace directives plus a
  missing-DTB failure after bootstrap. Firmware observes the selected tool files,
  embeds CARAPACE in PMI, and output reopening verifies that value equals the final
  published root and differs from the original tool root. Output has exactly one
  or four produced scutes and paired firmware; retained contents survive, temporary
  firmware files do not, and the formatter receipt is present. Failure publishes
  nothing and leaves no runtime attachment. Existing cached-base, firmware-only,
  transform, storage and lifecycle checks in the normal suite also pass.
- Host tests (87 library tests, integration suites, 52 doctests), strict root and
  fixture Clippy, formatting and diff checks pass. Evidence/source/executable hashes
  preserved under validation/rewrite-command-integration/evidence/v211. Full suite
  host observation timeout increased to 330 seconds for five additional real CLI
  builds; no production execution or cleanup deadline changed.
- Enabled networking/DNS is now the next major implementation gap: it is still
  explicitly rejected in pipeline and directive execution. General string-variable
  expansion audit, remaining stage/failure matrix and runtime acquisition faults,
  ORAS interoperability, three full M6 adherence passes, M7 qualification and M8
  retirement remain required. Runtime target directories are still author-supplied
  as recorded above. M6 stays active with zero clean adherence passes.
