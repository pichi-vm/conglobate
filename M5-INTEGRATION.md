# M5 direct-layer bootstrap implementation

M4 is accepted with three clean self-review passes. M5 is active with zero clean
passes. DESIGN.md, SANDBOX-DESIGN.txt and ARCHITECTURE.md remain authoritative.
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
