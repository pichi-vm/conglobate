# Foundation revalidation after workspace/context changes

Implementation scope: a7f7217. Status: two consecutive clean self-reviews.
This supplements the historical MILESTONE-3-ADHERENCE.md; it does not erase its
findings or reuse its three passes as acceptance of changed code. M3 must regain
three passes before further milestone advancement. M4 remains unfinished.

## Earlier revision findings

- 395ba80: context cleanup lost ownership on TempDir::close failure; corrected
  with a cleanup-only owner and EMFILE recovery in disposable-VM v77.
- a7f7217: failed population lost explicit cleanup authority; corrected with
  Before/Partial materialization errors and an actual partial-write regression.
- The milestone summary table incorrectly continued to show historical M3
  completion; it was reopened. Neither fix counted as a clean review.

## Clean pass 1: requirements and ownership traversal

Scope inspected at a7f7217; no source/test/harness edits during this review.

Requirements: revisited DESIGN §3 and SANDBOX-DESIGN §2.1/§3.3, plus the complete
M3 acceptance criteria and original review. Git diff against accepted foundation
621b34d confirms request/path/input, recipe/plan, cache/chain, lock/document/value,
template/copy/image sources are unchanged. Re-read request parsing, CapturedBuild
and ResolvedBuild transitions, lock resolution, cache authority, capture's verified
Git traversal and input-role planning. Required recipe/lock paths remain explicit;
committed inputs remain local, identity-checked, independent of working-tree edits;
unsupported inputs fail rather than fetch. Receipt semantics still do not promise
future cache bytes. Use-time layer verification is M4, not silently fulfilled by
M3's complete-read verification. Reviewed stage/null/default regression assertions.

Changed context: traced Workspace's detached tmpfs ownership, internal parent
clone, materialize_in's backing check, private directory allocation, ordered
population and root opening. Allocation/population failures cannot publish staged
success; Partial owns cleanup-only authority. Staged readers borrow the checkout
and expose no writable FD. Freeze consumes staging, clones a nonrecursive mount,
requires all read-only attributes before publication and closes its writable root.
Captured names cannot traverse out of the directory; confined read resolution
rejects symlink escapes. Cleanup retains its pinned parent on failure and cannot
return a valid checkout after partial removal. Read-only readers may outlive
explicit backing-name removal while retaining their own read-only mount view.

Rust style: inspected all changed context modules and storage. Sealed Checkout
markers and private state-specific resources remain intact. Constructors and
transitions are methods; safe file/mount library APIs enforce forbid(unsafe_code).
No public writable descriptor, Clone authority or mutable state flag was added.
Errors distinguish preallocation, partial population, freeze failure (original
staged state intact), and cleanup-only recovery. Local loops/counters are algorithm
state. TempDir fallback ordering keeps its proc-fd parent alive during removal;
Drop is not claimed as guaranteed success or process-death recovery.

Evidence: inspected v77's actual probe assertions and serial markers for EMFILE
setup retry, cleanup retry, EROFS before/after owner cleanup, retained reader bytes,
workspace parent lifetime and unchanged mountinfo. Inspected the host regression
for ENAMETOOLONG after a successful first file, retained partial directory and
explicit cleanup. Verified all 22 v77 provenance hashes and all 10 hashes in
context-population-review (including current reviewed source identities).
The preserved full-suite log passes, including 25 compile-fail checks; root and
fixture Clippy/check logs pass. No extra test run is counted as a review. v77
covers the earlier kernel transition; later error-handling changes have host
coverage, not a falsely relabeled VM build. The older harness was compiled after
API updates; its historical EBUSY behavior is not claimed for detached mounts.

Preservation and claims: diff shows no deleted reference, preservation or
validation files since 621b34d. Historical sources and failed attempts remain.
This review accepts foundation behavior only: no completed build CLI, assembled
volume, command integration, final inventory/output, distro or confidential-VM
qualification is implied. Runtime owner lifetimes and complete build assembly
remain M4/later obligations.

Result: no new finding; clean streak 1/3. This is source-grounded self-review,
not independent security review. Two further clean passes on unchanged scope are
required; any finding or relevant edit resets the streak.

## Clean pass 2: failure and evidence traversal

Implementation scope remains a7f7217. Confirmed an empty diff in src, tests,
Cargo files and the current VM harness since that scope. This pass began with
negative assertions and cleanup authority rather than the successful pipeline.

Failure inventory: before allocation, no context directory is published; once
allocated, failed opening/population returns Partial with cleanup-only authority.
The ENAMETOOLONG test verifies an actual earlier file exists, so it exercises
partial population rather than merely parser rejection. Frozen setup failure
leaves the original staged tree unchanged: the cloned detached mount is local
and dropped before returning the staged owner. Removal failure can partially
remove names, so its API exposes only retry, never staged/read-only recovery.
The parent FD remains owned through retries and destructor fallback; directory
cleanup precedes parent closure. Detached read-only roots close before backing
removal. Surviving file FDs remain read-only, as the VM checks demonstrate.

Reviewed the v77 probe's limit restoration before inspecting errors, successful
freeze retry, EROFS assertions before/after cleanup, exact-content checks, and
mountinfo comparison. These assertions qualify ordinary detached setup and
EMFILE recovery, not all storage faults or kernel-consumer lifetime. Current host
regression evidence separately covers the later population error. Re-ran only
context compile-fail doctests to inspect diagnostics: E0505 for consuming staging
with a borrowed reader, E0277 for moving the thread-affine ReadOnly state, E0599
for freezing ReadOnly twice. All fail for intended reasons. Diagnostics retained
in evidence/context-population-review/ownership-diagnostics.txt. Running this
check does not itself count as the source review.

Unchanged foundation failure paths: traced CheckedBlob and DecodedBlob finish
through length, digest, decoder completion and buffered trailing-byte checks;
verified chain candidate removal rejects duplicates/disconnection and terminates
at the declared zero parent. Inspected decoder-corruption tests which deliberately
use matching OCI digests, ensuring decoder errors are actually exercised. Cache
mutation tests distinguish captured metadata from subsequent reads. Host index
selection rejects incompatible/ambiguous entries and direct manifest pins. These
remain verification of completed input reads, not live mounted-layer guarantees.

Requirements and style: revisited duplicate/null/omitted YAML handling, finite
five-minute default timeouts, disabled networking, explicit resolver preflight,
from/input-role rules and the M3 boundary. No execution is inferred from a plan.
Reviewed public context/storage constructors and getters, sealed markers and
state payloads; raw mutable authority is not exported. Conversion/error types
represent real differences in available ownership. No runtime state flags,
freestanding production helpers or unsafe code were introduced. Read-only File
handles intentionally rely on the mount enforcement tested in the VM; staged
readers have stricter borrowing because pre-freeze mount authority is different.

Preservation/claims: only review records changed since the frozen implementation.
Earlier VM/source/failure evidence remains, without attributing v77 execution to
later builds. The public docs distinguish staging, read-only mount enforcement,
cleanup fallback and whole-build integration. M4 assembly, output, distro/hardware
qualification and final retirement remain explicitly incomplete.

Result: no new finding; clean streak 2/3. One further complete self-review of the
unchanged scope is required. This is not independent security qualification.
