# Binary simplification

Baseline: 8c2d294, clean tree; 18,587 production Rust code lines (tokei),
20,789 physical lines excluding trailing unit-test modules and firmware/tests.rs.
The baseline remains in Git and the prior verified archives.

Contract: standalone binary in a disposable VM. Preserve safe Rust, resource
ownership, useful typestate, isolated commands, finite deadlines, quiescence,
writeback, local verified inputs and atomic publication. Cleanup failure is
terminal and requires VM disposal; no build or storage recovery API is needed.
Use io::Result and propagation for I/O; anyhow at orchestration boundaries.
Context should identify a stage, command, image or path, not rename every call.

S1 baseline/plan: complete, three clean source reviews: (1) source concentration
and callers identify lifecycle/recovery as main target, (2) failure semantics
retain ownership until quiescence/disposal, (3) test migration preserves actual
assertions and records lost doctest coverage explicitly. No code changes yet.
S2 private binary/modules and test-only fixture: complete (3/3).
S3 error propagation and terminal cleanup: complete (3/3).
S4 lifecycle/assigned-volume consolidation: complete (3/3).
S5 docs, focused regressions, native architecture CI and measured reduction: pending.

Target about half the production code if justified by actual simplification.
Do not count moving code into another crate or deleting required checks as a
reduction. Three consecutive clean adherence reviews gate each milestone.

S2 implementation: removed library target; private modules shared only by the
normal binary and explicitly enabled VM fixture binary. Existing contract tests
are now binary-local unit tests (230 pass). Fixture compiles. Process-spawn test
selectors were updated for their new module-qualified names. Initial local test
run hit existing /tmp quota and stale selectors; rerun used owned storage TMPDIR.
Old compile-fail doctests are no longer executed by rustdoc (binary-only crate);
CI now compiles the VM fixture instead. No claim that this replaces negative
compiler assertions. Runtime invariants/tests remain and typestate still compiles.

S2 clean reviews: (1) Cargo targets and namespace visibility expose no library,
fixture is opt-in; (2) helper re-exec and guest guard unchanged, actual assertions
retained, selectors correct; (3) locked dependencies, 230 unit results and fixture
compilation verified, test packaging only. 3/3. Compiler unused-API warnings now
expose the removal candidates for S3/S4; final CI must be warning-free.


S3/S4 implementation was one dependent change: removing error-owned retry paths
requires changing the lifecycle that consumes them. Both gates were reviewed
against the complete resulting code before starting final CI qualification.

S3 removes build-error variants, cleanup histories and reusable failed storage
from pipeline, formatter, runtime, mounts, mappings, loop devices, verity, chain,
resolver staging, sealing and publication. I/O errors propagate directly;
anyhow adds stage/directive or combined failure context. Private child/admission
transition errors remain where immediate cancellation needs the child owner.
They do not escape as a failed-build recovery interface.

S4 removes Step reaping/draining/quiescent transition wrappers and retry phases.
Cleanup is linear. Networking is a retained field of Step. One CommandStorage
trait shares single-root and paired-root preparation, transfer, COPY and
finalization, replacing three overlapping traits and finalization adapters.
Mounted/unmounted state and actual storage/origin borrows remain. Unused public
library entry points and unexecuted library doctest snippets were removed;
fixture-only entry points are explicitly gated. Module declarations are direct
in the two binary roots so cargo fmt traverses production code. The command-probe
fixture is also an opt-in root-package binary; distro runners use that package.

Review findings fixed before the clean passes: retain snapshot validation after
unmount; check the original deadline before single-volume unmount; always attempt
cancellation/drain after a missing cleanup acknowledgment; migrate every runner
from the removed fixture manifest; preserve copyright headers. No new security
exceptions, syscall permissions, downloads, sleeps or host-privileged tests.

S3 clean adherence passes:
1. Source audit of I/O boundaries, partial setup and cleanup: errors no longer
   return reusable build/storage authority; operation and cleanup diagnostics
   survive. Private process transition owners serve immediate cleanup only.
2. Ownership audit through mounted and kernel-held resources: failed unmount
   retains mounts, mapping/loop destructor fallbacks never yield sealable backing,
   and failed runtime cleanup cannot publish. Cache-holder, formatter, runtime,
   launch rejection and writeback VM evidence supports these paths.
3. Acceptance audit: command outcome, cleanup acknowledgment, descendant drain,
   normal unmount, snapshot health and runtime close precede accepted output.
   Missing acknowledgment and successful-shell/busy-unmount fixtures reject;
   the full pipeline artifact verifier passes. 3/3.

S4 clean adherence passes:
1. State/API audit: one shared storage path, optional retained network, private
   linear cleanup, meaningful admission and mount states; no arbitrary success
   receipt construction or borrowed-root escape introduced.
2. Rust/style audit: methods own invariants, safe Rust remains forbidden, validated
   inputs remain immutable, RAII handles ordinary scope cleanup; unresolved live
   resources stay retained for VM disposal. Rootless Clippy passes with -D warnings.
3. Design/evidence audit: cache-backed immutable layers, one stage scute, separate
   bootstrap roots, firmware provenance, DTB binding, inventory and atomic rename
   remain intact. Integrated pipeline and DNS/resolver/offline-step tests pass;
   fixed teardown timeout and failed writeback reject completion. 3/3.

Rootless suite: 213 pass, versus 230 after S2. The 17 removed tests constructed
retired error variants/retry histories; the actual failure-path fixtures remain
and now check terminal rejection/retention. Tests of recovering and reusing failed
storage are intentionally retired. Resolver path confinement, inode replacement,
COPY, inventory, image verification and recipe tests remain. No claim of replacing
negative compiler tests with runtime tests.

Local records: /storage/conglobate-validation/simplification. Successful cases:
cache-terminal-v1, held-cleanup-v1, runtime-scope-v2, formatter-v2, gate-v3,
missing-cleanup-v3, launch-failure-v4, writeback-v4, cleanup-timeout-v4,
pipeline-v4 (artifact verifier), network-v4 (success verifier). Earlier snapshot
results apply to unchanged paths; source.tar.gz in each run identifies its exact
code. Pipeline-v3 stopped before VM launch for missing libbpf development flags;
v4 uses preserved headers and the installed library. A network verifier invoked
before guest completion was rerun against completed results; its preliminary
log is retained. These are not fresh four-distro boot qualifications.

Size audit uses the same script for baseline and final source, excluding unit-test
modules and fixture roots, without moving production logic into another crate.
The reproducible audit corrects the initial baseline estimate to 18,707 code
lines (20,913 physical); final code is 16,070 (17,734 physical): 14.10% fewer code
lines, 15.20% fewer physical lines. Test-only methods outside test modules remain
counted conservatively. The hoped-for half was not reached. Remaining code owns
actual protocol, filesystem/OCI/Git/DTB validation and sandbox checks; deleting
those to meet the target would change the agreed scope. Script and JSON counts
are retained with the evidence. Final CI and S5 reviews remain pending.
