Pinned runtime-directory prototype
===================================
Run runtime-path-fixed with guest.py, kernel 7.2.10-200.fc44.x86_64 and a new --out.
Run chain-runtime-pinned with the same arguments plus
--worker-build /tmp/conglobate-finalize-workers for the full FUSE build control.

The candidate launch script first makes propagation private and bind-mounts the
trusted /work/view location. Safe-Rust runtime-mounts opens that root O_PATH,
O_DIRECTORY, O_NOFOLLOW and O_CLOEXEC, then opens the fixed single-component
names proc, dev and .old relative to the root FD with the same constraints.
All three handles must exist before any runtime mount is attempted. proc/dev
mount targets are trusted /proc/self/fd links to those pinned directory handles.
Mount flags match the simple-chain launcher. Handles close when helper exits;
the entry helper then pivots, drops old root/FDs and applies the existing policy.

Eight small controls: directories accepted; proc/dev/.old symlinks, proc/dev
regular files and missing proc/dev paths rejected before target-pinned or mount
markers. Trusted inspection replaces entry for these small controls only; it
verifies mounts at /work/view/proc and /work/view/dev. Outside canary bytes and
mountinfo remain unchanged. Original unprotected behavior remains retained in
reproduced/runtime-path. This candidate does not alter the old fixture.

Required assumptions and remaining scope:
- Trusted stable root path and no image writers during setup/entry. This does
  not demonstrate arbitrary concurrent-renaming safety across mount and pivot.
- Only fixed top-level directory names; no arbitrary nested path traversal.
- Missing runtime directories fail here; safe temporary creation/restoration
  for images omitting them is not implemented or waived as a requirement.
- No devpts, dedicated device nodes, sys mount, resolver or tool/installroot
  dual-root setup in this simple-chain helper. Those variants remain to qualify.
- Mid-mount failure relies on private namespace teardown; no fault injection
  at those two mount calls is included in the eight path checks.
- This is an experimental launcher helper, not production executor adoption.

Results: eight small path checks pass (reproduced/runtime-path-fixed). Full actual
FUSE image run also passes (reproduced/chain-runtime-pinned): restricted command,
descendant teardown, sealing, authenticated restaging, kernel filesystem checks,
base preservation, inventory publication and final init/storage cleanup. The full
run uses the real entry helper; only the small matrix substitutes inspection.
This validates the candidate's stated no-writer/top-level-directory scope and
normal integration; remaining assumptions above are not promoted to passes.

Partial setup failure controls
-------------------------------
Cases chain-runtime-failure-proc and chain-runtime-failure-dev inject a trusted
helper error immediately after successful proc mount or after proc and dev
mounts, before entry/image execution. The actual FUSE/Chain/Snapshot/committed
input fixture runs with publication enabled. Both attempts reject worker status,
leave no sealed/verified/published output, preserve base and parent mountinfo,
and release FUSE connections, DM targets and loop devices. Namespace teardown
is the partial-mount cleanup mechanism. No manual mount/device removal fallback
is used by the test observer. These are injected errors after real successful
mounts, not failures returned by mount itself or asynchronous cancellation tests.

Evidence: reproduced/runtime-failure-proc and runtime-failure-dev. Initial proc
run omitted Git from guest packaging and stopped in input preparation; retained
as runtime-failure-fixture-negative, not a sandbox result. The corrected runner
packages Git for both cases. Current helper also passes all eight small path
controls without failure selection (runtime-path-fixed-regression). Earlier
full normal-build evidence remains pinned to its original helper source version.

Image-supplied symlink rejection through actual FUSE
---------------------------------------------------
Run chain-runtime-link-proc, chain-runtime-link-dev and chain-runtime-link-old
with the kernel/worker-build arguments above. Each creates an owned ext4 fixture
whose selected directory is a symlink to /work/canary, then imports/authenticates
it and enters the real mapped FUSE build path. The unmodified pinned-directory
helper rejects the actual FUSE-backed proc/dev/.old link with ENOTDIR. No failure
injection selector is passed. All three fail before target-pinned/runtime-mount
markers and image execution, sealing or output publication. Base, canary sentinel
and outer mountinfo hashes match; FUSE connections, DM targets and loops vanish.

Evidence: reproduced/runtime-link-proc, runtime-link-dev and runtime-link-old.
These complement the normal FUSE build and the small tmpfs rejection matrix;
no concurrent rename or unsupported nested/runtime-creation behavior is claimed.
