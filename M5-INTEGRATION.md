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
