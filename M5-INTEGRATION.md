# M5 direct-layer bootstrap implementation

M4 is accepted with three clean self-review passes. M5 is active with zero clean
passes. DESIGN.md, SANDBOX-DESIGN.txt and ARCHITECTURE.md remain authoritative.
The integrated VM fixture now demonstrates a verified tool chain, dual-root
commands through production aggregate preparation and execution acceptance,
direct output sealing and verified reopening. Initial metadata and formatter
failure qualification now pass; the comprehensive review gate remains.
This file records implementation order, not a substitute design or acceptance.

The end-to-end acceptance case is an actual verified tool carapace plus a fresh
64 GiB logical dm-zero origin with writable snapshot, trusted ext4 formatting of
the recipe-requested capacity, two isolated commands seeing tool `/` and output
`/mnt`, clean finalization, direct scute sealing, and reopening the emitted chain
to verify filesystem content/metadata. No intermediate filesystem-image import.

1. Trusted formatting and ownership. Reuse validated FilesystemSpec/FilesystemSize
   instead of reparsing recipe fields. Capture formatter executable, dependencies
   and builder-owned configuration as immutable execution inputs, then execute
   those captured bytes. The formatter receives only its assigned writable block
   view and trusted runtime. Carry configuration/profile/argument identities into
   an execution receipt only after successful bounded execution, flush and snapshot
   health validation. Failure retains the snapshot for cleanup and cannot yield
   formatted/accepted output. Use explicit 4096-byte blocks and size.blocks();
   preserve the fixed logical carapace geometry. Never accept recipe shell flags.
2. Dual-volume command ownership. Introduce a meaningful sealed StepStorage owner
   for tool/output volumes, with consuming preparation/finalization transitions.
   Extend trusted volume transfer/root setup to attach output at confined `/mnt`
   before restriction and admission. Bind both pinned filesystem identities into
   the existing two-target LSM policy. Preserve both volumes until descendants
   stop, remove policy, then release all views/pins and finalize both filesystems.
   One failure must retain the complete remaining resource set and original
   deadline. Remount both for every step; tool modifications persist only within
   bootstrap and never contribute layers to the output carapace.
3. Direct sealing. After clean finalization, detach the output COW backing and use
   snapshot/carapace wire/hash APIs (the carapace crate, without image conversion)
   to seal the actual persistent snapshot. Publishable stage output requires both
   accepted command execution and successful seal. Disposal of tool backing is
   separate from retention of output backing. Keep wire parsing out of this crate.
4. Qualification fixture. Extend the disposable-VM fixture to use real locked
   cached tool scutes, format the output directly, write/read across two commands,
   verify both authorized label targets and denied third tmpfs, then reopen output.
   Cover ownership/mode, ACLs, file capabilities and foreign SELinux labels in the
   output. Include failed formatter, exhausted backing and failure while one of
   two filesystems is still mounted. Keep source/logs for every failed attempt.
5. Review gate. Check design/style/type/cleanup/evidence/preservation across the
   whole integrated M5 result. Three consecutive clean passes are required before
   M6. A formatter argument builder or a dual-mount primitive alone is not M5.

Implementation should build toward this fixture early, avoiding another long
sequence of disconnected primitives. M6 adds the build coordinator's remaining
stages and publication; M7 still owns representative distro boots and actual
amd64/arm64 confidential-platform qualification.

## Formatter runtime capture started

CapturedFormatter captures a trusted-environment list of regular runtime files
into immutable measured bytes, sorted by normalized runtime path. The requested
executable must be present with execute permission. Resolve source symlinks inside
the supplied root; do not preserve host symlinks in the captured runtime. Reject
duplicates, missing files, special files and file/directory collisions. Keep
source path/descriptor authority out of the resulting object. Later staging must
materialize these bytes with ordinary executable/read-only permissions, removing
set-ID bits, and execute inside that captured root. This is not a formatting
receipt, automatic dependency discovery or a runnable formatter yet.

Tests establish source-mutation independence, confined absolute symlink resolution,
digest agreement and missing/duplicate/nonexecutable input rejection. Targeted
tests and strict Clippy pass. No privileged host operations were used.

Formatter option semantics checked against the e2fsprogs mke2fs manual:
https://man7.org/linux/man-pages/man8/mke2fs.8.html . The documented explicit block
count, root_owner, nodiscard, and lazy initialization controls match the existing
architecture contract. Versioned profile/configuration and isolated execution
remain to implement and validate in the disposable VM.

## Runtime staging (v157)

CapturedFormatter::stage consumes captured inputs into a FormatterRuntime on the
same build-wide tmpfs. A detached mount makes the runtime read-only, nosuid and
nodev while clearing inherited noexec for future trusted formatter execution.
Files receive only 0555 or 0444 permissions; no source set-ID bits propagate.
Writable source descriptors close before publication. Partial staging retains
backing for explicit cleanup; failed removal retains its owner for retry.
Closing the runtime drops its detached view before backing cleanup.

v157 verifies captured bytes despite changed source, stripped permissions,
EROFS on chmod through the read-only view, same device as workspace scratch and
successful close. Full normal integration suite and root/fixture Clippy pass.
This does not yet prove formatter execution or dependency completeness; those
require the next captured-root execution integration. No M5 adherence pass.

## Versioned ext4 profile

Ext4Format derives literal argument values from validated filesystem capacity and
label. The builder owns the mke2fs configuration and rejects captured files that
conflict with its configuration path, including ancestor/descendant collisions.
The profile selects ext4, a named usage type, 4096-byte blocks, fixed inode and
reserved-space defaults, root ownership, no discard and eager initialization.
Configuration and fixed arguments have a deterministic CBOR profile identity;
this describes inputs, not a successful execution receipt.

v158 and v159 passed the normal VM suite with the profile on a real writable
snapshot, but retained an implicit size-dependent `small` usage-type warning.
v160 explicitly selects `-T conglobate`. The fixture still executes the trusted
VM formatter at its original path: captured-root execution remains the next
integration requirement. No M5 adherence pass is claimed for this slice.

The next implementation should connect the captured runtime and assigned device
to a trusted helper with bounded execution and retained cleanup ownership, then
immediately use that path in the dual-volume bootstrap fixture. It must not
introduce another guardian or use synchronization sleeps.

## Captured formatter execution (v161–v163)

FormatterRuntime now consumes itself and the writable SnapshotMapping to launch
the trusted formatter helper. Formatting retains both resources and the child
wait owner. The helper receives detached runtime/device mounts over a private
descriptor packet, enters private mount/network/IPC/UTS namespaces, chroots into
the captured runtime and directly execs the captured executable. Its private dev
mount contains only the assigned output block device and null/random/urandom.
The runtime reserves dev; the parent view never gains the helper's submount.
The environment is cleared and the exact builder-owned configuration is required.

This is trusted setup, not image command execution: it retains initial-namespace
root and runs only the trusted formatter. The formatter must directly perform
its work; this path does not yet establish a descendant containment domain for
arbitrary trusted tools that fork. No recipe executable or shell is accepted.
Formatting uses the fixed setup cutoff. Wait failures retain process/storage/
runtime and a separate fixed cleanup cutoff; retry history preserves the original
failure. Reaped nonzero execution returns both resources for cleanup. Success
checks snapshot health, but is not a sealed-layer or inventory receipt.

v161 failed because the new fixture mode was dispatched inside an unrelated mode;
its source/logs are preserved. v162 executes the captured formatter with the
original executable absent, mounts the resulting ext4 and verifies the parent
runtime has no output device. v163 additionally omits all captured dependencies
and verifies execution fails without an ambient loader fallback, while recovering
both owners. The complete normal VM suite passes. Post-v163 review tightened the
cleanup cutoff and error-history handling; these edits have host checks, not a
new timeout-injection claim. Formatter timeout/cancellation, backing-exhaustion
and dual-volume failure qualification remain M5 work. No adherence pass claimed.

## Paired storage ownership (v164–v165)

BootstrapVolumes<Mounted/Unmounted> owns the disposable tool and output together.
Mounted pairs have no extraction method. Failed unmount retains both resources,
including the already-unmounted output if tool cleanup fails. Retry preserves
original diagnostics. Deadline-aware unmount checks before each operation and
completion, retains the cutoff in failure and cannot renew it.

Step::with_bootstrap retains the pair through existing descendant/policy teardown.
TeardownCompletion for that pair finalizes both and checks both snapshot statuses
under its original cutoff. This path does not manufacture command success. The
new Step finalization path still requires integrated command coverage; the VM
fixture below exercises the underlying pair, not that complete Step path.

The VM fixture formats two actual snapshots with the captured formatter, mounts
both and holds a file descriptor open first on output, then on tool. Both cases
reject unmount with EBUSY, retain the correct remaining mounts, preserve error
history on a second rejection, and finish after the descriptor closes. v164
passes the initial pair implementation. Source review then adds cutoff checks
between unmounts; v165 passes using the deadline-aware path. 47 compile-fail
doctests and root/fixture Clippy pass. Evidence and failed compile logs preserved.

Dual-view transfer, output attachment at /mnt, aggregate setup rollback, command
execution with both LSM targets and direct sealing still remain. M5 streak zero.

## Dual-view command execution (v166–v172)

A dedicated bootstrap control packet carries tool/output detached views and the
policy-bound user namespace together. The sender verifies both root identities
against its policy; received descriptors are close-on-exec and opcode/arity are
checked. The tool's reserved mnt directory is prepared in the original unmapped
filesystem before transfer, preserving image UID/GID 0. Existing symlinks or
non-directories at this reserved mountpoint fail rather than redirect attachment.
The helper maps both views, attaches the tool first and then the output at mnt,
retaining both owners on partial attachment failure. No command runs before this
setup and the existing readiness/admission gates. Cleanup unmounts output before
tool; the outer Step retains both backing volumes through descendant drain and
policy/pin removal.

Early runs failed before admission. v166/v167 exposed lost startup diagnostics;
v168 preserves receiver diagnostics after the setup child is reaped and reveals
EOVERFLOW from creating mnt through the idmapped view as host root. Preparing mnt
in the original view fixes that; v169 exposes EINVAL attaching output beneath a
still-detached tool view. Attaching tool first fixes that. v170 reaches commands
and clean teardown but the minimal tool fixture lacks chown. v171 adds required
fixture utilities and passes both commands. v172 repeats with the required 64GiB
logical devices, while formatting only 16MiB filesystems. All failures preserved.

The two commands write to tool and output independently, verify persistence after
remount, preserve output UID/GID 123:456 and foreign SELinux labels on both roots,
and reject labeling on a third tmpfs. Pair busy-unmount tests still pass. 53
library tests, 47 doctests and strict root/fixture Clippy pass. These runs exercise
production transfer, helper, Step, teardown and paired finalization, but the
fixture still assembles preparation itself and populates its tool root by trusted
copy. Production aggregate acquisition/rollback, a verified tool-carapace input,
direct sealing and expanded failure qualification remain. M5 clean streak zero.

## Direct sealing and verified tool input (v173–v175)

Scratch::seal_snapshot consumes detached COW storage after mapping removal and
confirmed loop detachment. Header/Exception decoding uses devmap-snapshot. The
used-extent walk retains all referenced data and the terminating metadata area,
including an extra area after a full exception chunk. It rejects invalid headers
and out-of-geometry references, then trims unused trailing capacity in place.
It preserves recorded zero writes: this is sealing a snapshot, not conversion of
a filesystem image. Content SHA-256 and verity metadata are generated in one
streaming pass over the retained extent. SealedScute exposes read-only cursors,
root/parent and descriptor; it is neither command-success nor publication proof.

The fixture writes a scute into a temporary local cache and uses production
index resolution, chain verification and dm-verity assembly to reopen it. v173
found malformed fixture annotations; v174 fixes them and verifies output bytes,
UID/GID and foreign SELinux label after reopening, with tool-state absent.
v175 first creates a tool artifact outside the bootstrap command sequence, then
executes both commands on a disposable overlay of that verified cache-backed
tool chain. The tool cache root remains unchanged. The independently formatted
output is sealed directly and reopened successfully. Both logical devices are
64GiB; their ext4 filesystems are 16MiB. No image-to-scute conversion occurs.

All 56 library tests pass, including extent-boundary, malformed-record and
independent read-only cursor checks; 48 doctests include rejection of writes through a
sealed reader. Root/fixture Clippy and the full normal VM suite pass. v173–v175
sources/logs/artifact hashes are preserved, including fixture metadata/lifetime
compile failures; post-v175 test-only source changes are identified separately.
Production aggregate setup and rollback still need integration, as do remaining
formatter/dual-volume failure tests and three complete adherence passes. No M5
acceptance or distro/confidential-platform qualification is claimed.

## Production pair mount acquisition and rollback (v176)

BootstrapVolumes<Unmounted>::mount_ext4_until now consumes both volumes under
one original setup cutoff. Failure before either mount, after mounting the tool,
or after mounting both retains the complete ownership state. Explicit rollback
uses the enclosing teardown cutoff; subsequent cleanup retries cannot renew it.
The existing paired unmount state machine handles partial acquisition without a
second cleanup implementation. No partially mounted pair can be extracted.

The disposable VM exercised invalid ext4 output after successfully mounting the
tool. An open tool-root handle forced EBUSY during rollback; the original EINVAL
remained primary, and retry after closing the handle recovered both volumes and
removed all mountpoints. An already-expired setup cutoff acquired no mounts.
Successful acquisition/unmount of both valid volumes also passed, followed by
the complete normal command suite, direct sealing and verified reopening.
Root and fixture strict Clippy pass; all 48 doctests pass. Evidence is preserved
under evidence/v176. Production namespace/pin/policy/domain acquisition for the
pair and command acceptance remain to integrate. M5 clean streak remains zero.


## Shared production preparation for bootstrap (v177–v178)

CommandSandbox::prepare_bootstrap now acquires the mapped namespace, both
filesystem mounts, both identity pins, containment domain and label policy.
StoragePreparation<R> and its owning failure share the single-volume lifecycle;
sealed PreparationStorage implementations preserve each layout's concrete
unmounted and cleanup types. Existing single-volume public names are aliases.
Both layouts use the same fixed setup/rollback cutoffs and retain storage through
process termination, descendant drain, policy removal and identity release.
Partial mount acquisition and partial pin acquisition have explicit owning
rollback states. Original mount errors cross the generic boundary without
formatting away their concrete payloads.

The bootstrap fixture now calls this production entry point for both commands.
Three separate rejected-build processes exercise domain creation, policy
installation and helper launch failures, then verify both volumes are recovered
and all recovery pins, policy entries and mountpoints are gone. v177 first found
a fixture error: it tried to reinstall the exclusive subreaper after terminal
rollback. v178 isolates each rejected build in its own process, preserving the
production one-installation rule. Both failed and successful evidence is saved.
The full v178 VM suite passes, including two successful bootstrap commands,
label persistence and direct output sealing/reopening. All 56 library tests,
48 doctests and root/fixture strict Clippy pass.

Execution acceptance is still assembled by the fixture; it must become a
production receipt tied to the pair's own command outcome and finalization.
Remaining formatter cancellation/exhaustion and metadata qualification, plus
three full adherence passes, are still required. No M5 acceptance is claimed.


## Production execution acceptance for both volumes (v179–v180)

Step::execute now shares one acceptance path for single-volume and bootstrap
storage through sealed ExecutionStorage implementations. ExecutedBootstrap is
an immutable success receipt created only after the command's successful exit,
descendant quiescence, policy/pin cleanup, both filesystem unmounts and both
snapshot health checks under the original teardown cutoff. StorageExecutionFailure
retains the same primary outcome and concrete cleanup owner across retries.
Rejected storage cannot be extracted before finalization, and a completed cleanup
cannot convert a nonzero exit or timeout into success. Existing single-volume
names remain concrete aliases; no separate bootstrap acceptance state machine.

The fixture's two successful commands now use production prepare/launch/execute,
then seal and reopen the output. Four separate cases prove exit 17 rejection,
100ms command timeout rejection, retained nonzero outcome through busy-output
cleanup retries, and successful receipt issuance only after a busy-output handle
is released and finalization succeeds. v179 rejected an invalid timeout fixture
that assumed /bin/sleep existed; v180 uses shell builtins and passes the entire
normal VM suite. No timing sleep is used for synchronization.

All 56 library tests and 49 doctests pass; root and fixture strict Clippy pass.
Post-run doctest-only edits strengthen receipt-construction checks to provide
all fields, ensuring privacy is the reason compilation fails. Exact booted
source and the post-run doctest source are separately preserved. Remaining
formatter cancellation/exhaustion and metadata qualification and three full
adherence passes remain; M5 is not accepted.


## Metadata fidelity and formatter failures (v181–v182)

v181 expands the two-command bootstrap to write an ACL with numeric UID789,
mode0640 and owner123:456, plus a mode0755 executable with
cap_net_bind_service=ep. The second command checks ACL/capability persistence
after remount. Reopening the sealed verified chain outside the command namespace
checks exact numeric ACL entries, modes/ownership, the capability without a
nonzero build-root identity, and the foreign SELinux label. Full normal VM suite
passes. This proves metadata fidelity for these values, not boot-time capability
execution or distro qualification.

v182 is a separate targeted VM run so it can exercise the real fixed 30-second
formatter setup cutoff without extending the normal suite's observation timeout.
A captured trusted fixture executable parks without writing. WaitError::Deadline
is preserved; explicit cancellation reaps it, the child list is empty, and the
runtime/snapshot return for cleanup. A real captured mke2fs run on 64KiB COW
backing exhausts the snapshot and cannot yield FormattedSnapshot; usage reports
InvalidData. Runtime, snapshot mapping, loop and origin cleanup all succeed.
The fixture stall uses no synchronization sleep and is guarded by the disposable
VM marker, also included in its captured runtime. Strict fixture Clippy passes.

Only validation code changed. v181/v182 retain exact sources, logs and artifact
hashes; v182 is not represented as a full normal suite run. These results close
the listed initial metadata and formatter cancellation/exhaustion checks. Full
M5 design/style/ownership/evidence review, findings resolution and three clean
passes remain before acceptance; milestone clean streak remains zero.


## Initial adherence review finding R1 (v183–v186)

Source review found that Formatting's ordinary input box could drop after
Process's bounded best-effort destructor without explicit reaping proof. That
could delete the captured runtime and request snapshot teardown while the process
remained unresolved. Inputs now use safe ManuallyDrop retention until successful
wait or cancellation, matching the command lifecycle's VM-disposal fallback.
Launch failures still return inputs because no child was created; completion
failures follow a successful reap and may return inputs normally.

v183 revalidates timeout/cancel/reap and exhausted backing. v184 and v185 are
terminal disposable-VM drop tests for running and timed-out formatter owners;
the snapshot mapping remains present after the owner is dropped. v186 passes the
full normal VM suite, including successful formatting and bootstrap metadata/seal
checks. All 56 library tests, 49 doctests and root/fixture strict Clippy pass.
Exact evidence is preserved. R1 is addressed; the initial review is non-clean.
See preservation/MILESTONE-5-ADHERENCE.md for coverage and remaining review work.
No clean adherence pass is claimed; M5 remains at zero.
