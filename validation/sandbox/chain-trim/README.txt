Bounded FITRIM on the restricted FUSE/snapshot root
=================================================
Reproduce:
  python3 validation/sandbox/guest.py chain-trim \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-chain-trim --run

Disposable guest, no host disks/shares/NIC. The actual authenticated base ->
Snapshot -> FUSE -> restricted command -> sealed/reconstructed layer -> Record
publication fixture includes a tiny C compatibility probe. Test C only; production
Rust remains unsafe-free. The probe opens only its own image '/' directory and
requests FITRIM over the known 64MiB fixture extent with 4096-byte minimum range.
No arbitrary device is opened, no special malformed range is used, and no host
filesystem is targeted. Kernel/inputs, compiler command and source are retained.

Observed: FITRIM returns EOPNOTSUPP (95). The diagnostic deliberately classifies
unsupported/permission outcomes and continues so filesystem/base preservation
can be verified; this must NOT be reported as a successful trim or as transparent
compatibility with a recipe that treats trim failure as fatal. An unexpected I/O
error fails the probe. Future outcomes remain visible in the result marker.

Subsequent actual layer sealing, authenticated restaging, kernel e2fsck/remount,
file content/ownership checks, unchanged base hashes and inventory publication
pass. Final DM/loop cleanup passes. Evidence: reproduced/chain-trim.

src/snapshot.rs selects Discard::Disabled. That is consistent with the unsupported
result but no syscall/backend trace attributes errno 95 to a particular layer.
No claim is made about raw-loop FUSE, a discard-enabled snapshot, other filesystems
or metadata authorization in a different namespace arrangement. Default FUSE
journal limitations remain unchanged. This validates bounded compatibility and
preservation after the observed unsupported operation, not effective trimming.
