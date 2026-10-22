Worker finalization to real scute/inventory/OCI publication
=========================================================

Build baseline/patched workers as described in ../finalize/README.txt, then:
  python3 validation/sandbox/guest.py publication \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/NEW-WORKERS --out /tmp/NEW-PUBLICATION-GUEST --run

The safe-Rust helper imports the real repository Scute, Artifact and Inventory
implementations. Its independent Cargo.lock pins the probe dependencies; no
production dependency changes are made. The existing copy-helper target
directory is reused only as a compiler cache. Source hashes, packaged executable
hashes and the guest command are retained.

Guest-only sequence:
1. Create a new tmpfs ext4 image and a linear dm/loop mapping.
2. Mount using the mapped-userns trusted FUSE worker; write a small fixture.
3. Apply the selected diagnostic/real I/O fault, if any.
4. Unmount and reap the FUSE worker; obtain its actual exit status.
5. Remove dm/loop mappings, revoke path ownership/write permission.
6. Reject nonzero worker status before scute conversion or artifact creation.
7. For successful status, fsck, call Scute::import, stream into Artifact,
   finalize a synthetic inventory and publish a self-contained OCI layout.
8. Decode/re-encode inventory, verify the published scute digest/size, and check
   that the raw image digest did not change during conversion/publication.

Observed matrix:
  Patched worker clean                         Publish; real format checks pass
  Baseline diagnostic close EIO, exit0          Publish — NEGATIVE CONTROL
  Patched diagnostic close EIO, exit32          Reject before conversion
  Patched real final flush EIO, exit32          Reject before conversion
  Patched fsync EIO then device recovery, exit32 Reject before conversion
  Clean worker, artifact staging tmpfs 4KiB     StorageFull; no destination
  Clean worker, publication destination 4KiB    StorageFull; no destination
                                                or temporary copy directory

The close-only diagnostic wrapper calls the real close then returns EIO, so its
image remains fsck-clean. Baseline false success admits publication despite
that reported error. This demonstrates the contract gap, not media corruption.
The patched worker's failure gate prevents that publication. Real I/O cases
switch only a guest dm target to error; no host disks/devices are exposed.

Exhaustion checks require StorageFull specifically, not any generic failure.
The final-copy case has an already finalized private inventory before failure;
the external destination remains absent and its parent has no temporary residue.
The fixture supervisor (not command code) supplies worker status to the gate.

The inventory is explicitly synthetic: builder version identifies the probe;
context commit/tree and refs.lock are test values, not a real checkout or
attestation. Its schema requires the builder name conglobate. This validates
format/output binding, not trustworthy context-input measurement.

Limits:
- No untrusted recipe process or authenticated base carapace is present here.
  This connects worker teardown to actual bootstrap-style scute/publication
  APIs; it does not complete the authenticated extend-a-base pipeline.
- Writable mappings are removed and worker FDs are closed before conversion.
  Changing mode alone would not revoke open writer FDs; the earlier negative
  control remains applicable.
- The shell supervisor is experimental. Production build.rs does not acquire
  these guarantees merely because the fixture passes.
- Full cancellation, supervisor/guardian failure, delayed requests, concurrency,
  request pressure, and tmpfs exhaustion of every build phase remain open.
- Neither this patch nor the backend is adopted. Other ACL/metadata defects
  remain, and no attestation is produced.

Evidence:
  ../reproduced/publication/{inputs.json,command.json,results.txt,source-inputs.json}
  ../reproduced/finalize/build-inputs.json (external worker build identities)
