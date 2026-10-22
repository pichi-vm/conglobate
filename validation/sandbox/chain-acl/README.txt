Protocol-corrected ACL worker in the authenticated publication path
==================================================================
Build the finalization workers and the acl-protocol pair using their READMEs.
Run:
  python3 validation/sandbox/guest.py chain-acl \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/FINALIZE-WORKERS \
    --protocol-worker-build /tmp/ACL-PROTOCOL-PAIR \
    --out /tmp/NEW-CHAIN-ACL --run

This extends chain-publication with the corrected worker/library, including
its initialization-error status correction. The --protocol-worker-build option
is explicitly scoped to this fixture; it does not change ordinary chain cases.
The trusted executable at /patched and libfuse3.so.4 are replaced together.
The remaining baseline diagnostic binaries come from the finalization fixture.

Before import, the base receives setfacl/getfacl and their runtime alongside
the measured command. After resolving the actual committed YAML/OCI-index lock,
the restricted command creates a file with a named-user ACL, revokes/restores
its mask through chmod600/640 and checks the resulting ACL. The command has
no old-root/context/backing-device path. Writer quiescence and worker teardown
precede sealing. Kernel ext4 remount verifies content, mode640, named-user entry
and restored mask on the resulting filesystem. Prior chain checks, unchanged
base hashes and post-stage corruption/EIO/restoration checks remain enabled.

Actual Record/Artifact inventory and publication then complete. Published CBOR
round-trips and all published scutes verify/reconstruct. Evidence and local
source snapshots: ../reproduced/chain-acl. The packaged worker/library hashes
were checked against the corrected pair's build record, retained alongside it.

Scope: a fixed offline command using ACL tools, not a representative OS build
or complete policy/ACL qualification. The separate acl-protocol differential
matrix establishes the specific CAP_FSETID cases; this test connects that pair
to restricted execution and authenticated output. Fault/cancellation behavior
with this pair, wider workloads, maintenance and other metadata blockers remain
open. No production backend or executor is selected by this experiment.
