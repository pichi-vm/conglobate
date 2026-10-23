# M4 acquisition and rollback inspection

Component review, not a full clean adherence pass. Sources: step/preparation.rs,
step/preparation/sandbox.rs, policy/installation.rs, policy/pins.rs,
domain/recorded/setup.rs, step/setup.rs and the integrated fixture.

The namespace creator runs trusted code before command containment exists.
Launch/mapping/wait failures retain the creator process where needed. Rollback
terminates it and drains adopted children. After creator exit, failures to pin
the namespace retain any partially created target. Mount failure returns the
unmounted volume; filesystem-pin failure retains both mounted volume and target.
No image command has executed in these phases.

Containment creation precedes policy installation. Publication failure compares
an existing record with the identity of the owned domain: absent, exact and
malformed/mismatched states have distinct handling. The latter retain the domain
instead of deleting an unrelated record. Installed policy remains disabled
through binding and launch. Every helper-launch failure retains its child (if
created), cancellation owner, reaper, disabled policy/identities and volume.

Rollback's phases encode resources still held, not successful commands. It
terminates/reaps pre-admission helper processes, cancels their domain, drains
children, removes the record, closes disabled policy, releases pins, then
flushes/unmounts storage. Each transition reuses the original rollback deadline;
returning late from a kernel operation cannot yield a successful rollback.
RolledBackPreparation contains diagnostics and rejected storage, never an
ExecutedVolume. Phase-specific runtime enums are required to represent fallible
cleanup; the caller cannot select or mutate those phases.

Found R4: a post-PinDirectory-creation deadline check dropped the directory via
best-effort Drop and returned Pending::None. Changed the failure to retain
Pending::Directory, with consuming explicit close and ownership retained on
removal error. The unpinned loaded BPF object may close normally: no links were
attached, and policy never became active. Partial pin/link failures already
retain DisabledPinRecovery, while binding and late bound-policy failures retain
their exact owners and identities.

Evidence includes namespace quota/mount setup failures (v145 onward), domain
publication absent/exact/malformed/mismatch (v140 onward), policy-installation
failure (v136 onward), helper launch/exchange/registration failures (v134 onward),
and policy/unmount retry with retained diagnostics. These are targeted paths,
not exhaustive failure injection at every syscall. R4's precise deadline boundary
is source-reviewed, not deterministically injected in a VM. Host checks and a
current normal VM regression run accompany the correction.

Public construction APIs are trusted-supervisor APIs, not recipe controls. The
complete build coordinator must use one SetupBudget, the same identity allocation
for receiver/preparation, and the gated Step::execute path. It must not substitute
low-level caller-supplied deadlines. Current VM integration demonstrates that
composition; complete CLI/pipeline assembly remains later milestone work.
