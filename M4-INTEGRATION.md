# M4 integration review

Reviewed baseline: `78eaf96`. This is a focused source review with findings,
not a clean milestone adherence pass. M4 remains at zero clean passes.

The primitives have substantial disposable-VM coverage. The production executor
is not assembled: Cargo currently builds a library, and command orchestration
lives in `validation/rewrite-command-integration/src/main.rs`. A passing fixture
is evidence for the exercised primitives, not a working conglobate build binary.

## Findings that block M4 acceptance

| ID | Source evidence | Required resolution |
| --- | --- | --- |
| I1 | `Integration::run` implements `volume-receiver`, `gated-namespaces`, `gated-root` and the parent launch sequence. `src/request.rs` parses public build inputs but performs no execution. | Move the ordinary trusted setup/launch sequence into production owners and helper entry modes. Keep injected faults and assertions in the fixture. Exercise the production entry modes in the VM. |
| I2 | The parent creates `SetupBudget`, but namespace creation still gets a fresh five seconds. The receiver starts another five-second cutoff; root-entry starts another thirty seconds. `SetupBudget::await_prepared` checks the original parent cutoff only after these acquisitions. | Carry one setup cutoff through all trusted helper transitions. Check before starting work and after operations return. Late completion must retain acquired resources for cleanup and must not admit a command. Do not reset the budget when a helper starts. |
| I3 | Both namespace launcher and volume receiver wait for their command child with `Instant::now() + 30s`. | These fixture waits must not become production command limits. The supervisor owns the recipe execution deadline, including the five-minute default and longer authored durations. Helpers observe child completion and supervisor cancellation without imposing an independent shorter command timeout. |
| I4 | `Step::new` is reached only after namespace creation/pinning, volume mounting, policy load/pin/bind and contained-helper launch. Those operations are composed with fixture `?` and string conversions. | Add a preparation owner that retains each acquired resource on failure, including typed launch/binding failures. Before admission, explicitly cancel/reap any launched helper, remove disabled policy while identities are held, release pins, and finalize assigned storage. Keep primary setup error separate from cleanup diagnostics. |
| I5 | The fixture hardcodes UID/GID 100000, external `/work` paths, block-device nodes and fixture formatting/population. | Production launch must receive validated identity allocation and owned paths/handles. Do not embed fixture conventions or `mke2fs`/`cp -a` test population in the executor. Typed formatting is M5; confined COPY is M6. |
| I6 | `CommandResult` preserves command outcomes and cleanup owners, while low-level `Step::admit_until`, `Step::disable`, and `TeardownCompletion` remain independently accessible. | The orchestrator must use the gated path and require both a successful command outcome and clean storage finalization before returning a successful step receipt. Cleanup success alone must never authorize sealing. Review low-level visibility once production call sites exist. |
| I7 | Earlier guardian types still exist in `src/domain/guardian.rs` and `src/process/supervision/guardian.rs`. The current owned-volume path does not require them. | Do not introduce a guardian dependency into extraction. Preserve historical evidence; retire obsolete active APIs when their remaining validation callers are migrated, consistent with M8 preservation requirements. |

## Production ownership boundary

The supervisor remains the only build coordinator. Trusted setup helpers are
short-lived process roles needed to enter namespaces and create PID 1. They do
not survive supervisor loss to recover or resume the build.

A preparation owner should consume the assigned mounted volume, mapped identity
allocation and fixed setup budget. It acquires pins, disabled policy and a
contained helper, then becomes the existing ready step. Each fallible acquisition
must leave either the next typed owner or an error owning the partially prepared
resources. Keep pre-admission cleanup distinct from command teardown: policy is
still disabled, but a launched helper still needs cancellation and reaping.

The ready step transfers its root view, waits for restricted-leaf readiness, and
starts the existing `RunningCommand`. The execution clock starts at the start
request. `CommandResult` then carries the observation through teardown and volume
finalization. A separate successful step receipt may be constructed only after
checking the outcome and storage result together. It is not a sealed image.

The helper protocol must carry setup timing explicitly. It must not serialize
Rust `Instant` internals or send a remaining duration that silently renews on
receipt. Choose a single monotonic clock representation within the VM and retain
an absolute cutoff, or keep timeout enforcement wholly in the supervisor with
explicit cancellation of every helper. Resolve this before copying the fixture's
fresh deadlines into production. No sleep establishes readiness or termination.

## Next implementation slice and evidence

1. Extract ordinary trusted helper roles into production, with explicit immutable
   launch inputs and inherited descriptor transport. Retain fault injection in
   the fixture. Do not extract its hardcoded paths, identities, formatting or
   negative bind probes.
2. Make setup cutoff propagation and supervisor-owned execution timing part of
   that extraction. Validate delayed readiness using event-controlled helpers,
   immediate expiry, cancellation before readiness, and peer loss. A test that
   only finishes in less than thirty seconds cannot prove I3 resolved.
3. Introduce the preparation owner and exercise failure after each acquisition.
   Check retained identity/storage ownership and cleanup order, not only an error
   string or absence of a success marker.
4. Assemble the single-volume production step, including successful receipt
   gating. Re-run the existing label, descriptor, descendant, timeout, cleanup
   failure and supervisor-death cases through that path.
5. Perform the full M4 adherence reviews. This focused review does not cover every
   M4 source/error path and cannot count toward the three clean passes.

M5–M8 remain required. This review neither moves their scope into M4 nor treats
unavailable architecture/confidential-platform qualification as passed.
