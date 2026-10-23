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

## Extraction progress after review

`GatedCommandSetup::execute` now owns ordinary namespace entry, startup input
transfer, child launch and child status collection. Its crate-private process
wait has no independent execution cutoff; it is used only inside the supervised
command domain. The build supervisor must retain its finite deadline and domain
cancellation. This is not an unbounded supervisor wait API.

`GatedRootSetup::restrict` now owns ordinary private mount namespace creation,
root entry, capability/seccomp restriction and retention of the start endpoint.
The returned `GatedRestrictedCommand` consumes itself at start. The fixture uses
these production methods while retaining its extra-descriptor fault injection.

I1 and I3 are only partially resolved: the outer volume receiver is still fixture
orchestration with a thirty-second child wait, and the full binary dispatcher is
not implemented. I2, I4–I7 remain open. Short success/cancellation tests do not
prove support for all authored durations; do not close the timing finding yet.


The receiver's gated launch/readiness/admission/child-wait/completion sequence now
lives in `CommandReceiver`. It consumes the registered channel, attached mount
and command setup; failure variants retain the attached or in-use mount and any
child wait ownership. Its child wait is supervisor-controlled. Completion status
delivery has a fresh fixed internal communication deadline instead of reusing
an expired setup cutoff. The supervisor still rejects completion observed after
the recipe execution deadline. I3's independent child execution cutoffs are now
removed from this path; I2's cross-process setup cutoff propagation remains open.
Initial receiver input decoding, mount/device preparation, helper dispatch and
parent acquisition orchestration are still fixture code, so I1/I4 remain open.


Setup cutoff propagation now uses `SetupCutoff`, an absolute CLOCK_MONOTONIC
nanosecond value serialized as deterministic CBOR [1, value] (hex for helper
argv). Export samples monotonic time before measuring the remaining Instant
budget; import samples Instant before measuring monotonic remaining time. Both
orders are conservative and never renew the budget. The helper chain forwards
the same encoded cutoff; namespace and restricted-root preparation check it,
and restricted start derives its deadline from that retained value. This relies
on the helpers sharing the VM's time namespace; CommandNamespaces does not
create a time namespace. It is not a timestamp transferable to another VM.

I2's fresh helper deadlines are removed from the owned-volume execution path,
including initial user-namespace creation. Comprehensive pre/post acquisition
checks and failure ownership before the ready step remain part of I4, rather
than being claimed complete by the clock transport. The public binary dispatcher
and immutable launch-input transport still need integration.


The policy activation map handle is now acquired during disabled-policy binding,
which already retains policy and identity owners on failure. BoundResources owns
that handle through activation/disable and retains it on cleanup failure. It is
closed before identities are returned after successful policy cleanup.
`Step::new` is consequently an infallible ownership transfer; the old post-helper
PrepareFailure state is removed. This reduces I4's fallible preparation surface,
but does not replace the missing aggregate for earlier acquisitions or helper
launch failure cleanup.

### Production helper dispatch (v130–v131)

The production execution module now owns immutable, versioned helper launch
inputs and dispatch for namespace creation, volume reception, namespace entry
and restricted root entry. All transitions carry the same absolute setup
cutoff. The executable is supplied by the trusted caller; no helper is copied
into the command filesystem. Launch inputs are bounded and canonical CBOR
encoded for private argv transport, with receiver IDs and mount parent supplied
explicitly rather than taken from fixture constants.

Ordinary owned-volume fixture cases now call this dispatcher, including long
commands, timeout, nonzero exit, cleanup retry and supervisor death. Only
intentional setup and descriptor faults use the fixture helper branches.
v130 passes the normal suite. The public build executable and parent acquisition
coordinator are still absent: this is production library dispatch exercised by
the fixture executable, not a complete CLI. I1 is reduced, not closed. I4's
aggregate preparation failure owner and I6's success receipt remain required.
No M4 adherence pass is claimed.

- v131 supervisor-death suite passes through the production dispatcher; the
  independent controller verifies retained live-domain policy/storage and
  shuts down the VM. Source, host checks and artifact hashes are archived.

### Aggregate ownership across helper launch (v132 onward)

CommandPreparation now consumes the bound disabled policy, assigned mounted
volume, recorded domain and installed child reaper before launching a helper.
Success transfers them directly into Step<Ready>. Startup failure produces a
cleanup-only PreparationFailure, retaining any unregistered or identified child.
Rollback terminates/reaps that child, cancels its domain, drains descendants,
removes the record, removes the disabled policy, releases identity pins and
flushes/unmounts storage. A single fixed thirty-second cleanup budget survives
all retries. Policy and storage are deliberately retained on abandonment before
quiescence, under the dedicated-VM disposal contract.

The rollback result retains the original launch error and ordered cleanup
errors. It is explicitly a failed preparation result, not a successful command
receipt. Volume cleanup errors retain their concrete shared error payloads.
v132 tests missing executable and identity-exchange rejection, each combined
with a blocked policy-directory cleanup followed by a busy unmount; both
failures recover without losing the original error or remaining resources.

This addresses the launch boundary in I4. It does not yet aggregate namespace
creation/pinning, mount construction, policy load/pin/bind, or domain creation.
Those acquisitions still precede CommandPreparation in the fixture. Transfer/
readiness failures after launch still require caller-driven Step teardown.
I4 therefore remains open, as do production orchestration and receipt gating.

- v133 adds registration-rejection rollback coverage and passes the normal
  suite. v134 passes supervisor-death regression through the aggregate launch
  owner. All three startup failure variants retain their primary error across
  policy and unmount cleanup retries. Evidence/source hashes are verified.

### Integrated command execution and acceptance (v135)

Step<Ready, Volume<Mounted>>::execute now owns the post-launch protocol:
assigned-volume transfer, restricted-leaf readiness, bounded admission,
completion observation, teardown and storage finalization. Setup/admission
errors go through the same teardown owners, rather than escaping through a
fixture string conversion. ExecutionFailure retains the primary setup/command
outcome and a typed cleanup owner. Retry preserves the original teardown
deadline; fully cleaned-up rejected storage remains distinguishable from success.

ExecutedVolume is an opaque receipt created only inside this execution path
after successful command completion and finalization of the same assigned
volume. It cannot be constructed from an arbitrary CommandResult::map_cleanup
combination or an unrelated unmounted volume. It is not a layer seal.
The v135 VM suite tests exit 0, exit 37, timeout and pre-readiness helper failure,
each combined with a forced EBUSY unmount and subsequent retry. Only exit 0
produces the receipt. Rejected commands retain their failure even after all
cleanup succeeds; storage cannot escape before finalization.

This implements the I6 acceptance rule for the integrated path. A full low-level
API visibility review remains required; the public CLI/build coordinator and
earlier acquisition aggregate are still absent. Other fixture cases remain
deliberately decomposed for fault injection and inspection. M4 has no clean
adherence pass yet.

### Production policy installation and rollback (v136 onward)

PolicyInstallation owns namespace/filesystem pins while constructing the
embedded policy, loading it disabled, creating its protected pin directory,
attaching/pinning guards and binding the identities. Acquisitions check the
original setup cutoff before proceeding and after returning. Installation
requires the supervisor's initial user namespace; the trusted storage path
must still establish that it created the assigned superblocks there.

CommandPreparation::install incorporates installation failure into the existing
domain/descendant/policy/pin/storage rollback owner. Partial disabled attachments
and failed binding retain their policy resources and identity pins until
explicit cleanup. A late successful binding likewise returns cleanup-only
ownership. No failure returns an executable preparation.

Ordinary fixture cases now use production installation; the success fixture
case retains independent malformed-bind/descriptor-exhaustion probes. v136
passes the normal suite and a missing-policy-parent failure after object load.
The failure keeps ENOENT through a forced EBUSY unmount/retry, verifies domain and
identity records disappear before storage release, and closes the writable
snapshot normally. This does not independently exercise every partial attach
or late-binding error in the new aggregate.

Namespace creation/pinning, filesystem mount/pinning and recorded-domain
creation still precede CommandPreparation. In particular, domain publication
failure may leave a complete record and must be reconciled under its retained
lease, rather than discarded through a string error. These remain I4 work.
No full M4 adherence pass is claimed.

- v137 supervisor-death regression passes through production policy installation.
  Its source/artifact hashes and independent-controller shutdown evidence are
  preserved; this does not constitute a full adherence or qualification pass.
