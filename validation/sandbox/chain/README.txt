Authenticated carapace through FUSE and a new verified layer
==========================================================

Build the external workers using ../finalize/README.txt, then:
  python3 validation/sandbox/guest.py chain \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/NEW-WORKERS --out /tmp/NEW-CHAIN-GUEST --run

The safe-Rust chain binary lives in ../publication/helper/src/bin/chain.rs and
uses that probe's lockfile. It calls the actual repository Scute::import,
Chain::stage/assemble and Snapshot::create/finish APIs. The worker is the
experimental finalization-corrected fuse2fs, not the separate ACL correction.
The guest has no host shares, disks or NICs.

Bounded sequence:
- Create a 64MiB ext4 image with two small base files.
- Convert to a scute and cache blob; pin its calculated carapace root.
- Reject a wrong root and a one-byte cache mutation before assembly.
- Stage and assemble the verified base through carapace/dm-verity.
- Create the real persistent writable snapshot on that base.
- Give only the fixture snapshot device to the mapped-userns FUSE worker.
  Through FUSE, preserve one file, change another and create a third with
  target UID1234/GID2345/mode0640.
- Unmount and reap the worker, then Snapshot::finish detaches and seals.
- Verify the derived parent root and that the resulting root changed.
- Cache both layers with reversed manifest order, stage/reconstruct the chain,
  assemble it, and read the 64MiB filesystem extent through the kernel devices.
- fsck and ordinary read-only kernel ext4 mount verify all files and metadata.
- Mutate one byte in a private staged derived data block, leaving its verity
  tree unchanged. Assembly/read returns EIO. Restore the byte; a fresh assembly
  and full extent read succeed.
- Verify both original raw-image and base-scute digests are unchanged.

Evidence:
  ../reproduced/chain/{inputs.json,command.json,results.txt,source-inputs.json}
  ../reproduced/finalize/build-inputs.json (external worker build)

The post-stage mutation is a trusted diagnostic on a disposable private copy.
It is not an attempt to modify any outside image. Requiring EIO specifically,
then a successful restored read, distinguishes this from accepting an unrelated
setup failure as an integrity pass.

Limits:
- This establishes a bounded base -> authenticated device -> snapshot/FUSE ->
  sealed layer -> reverified chain path. It is not a full recipe execution.
- The base root is generated and pinned by the fixture. Git context, refs.lock,
  registry index selection and inventory trust are not part of this test.
- No untrusted image program runs; the trusted worker script performs writes.
  The restricted-command and publication fixtures remain separate evidence.
- Only a 64MiB filesystem extent is materialized from the larger logical
  carapace volume. Other filesystem types, larger chains and exhaustive
  structured metadata corruption still need qualification.
- Device/loop permissions are guest-only. The production executor and backend
  selection are unchanged.
