Repository scute integration tests, including privileged cases
==============================================================
Reproduce:
  python3 validation/sandbox/guest.py scute-tests \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --out /tmp/NEW-scute-tests --run

Runner builds the actual tests/scutes.rs binary using Cargo's JSON artifact
record, records compile command/log and source hashes, and packages it into a
disposable guest. No host disks/shares/NIC. Guest mounts tmpfs at /tmp, makes
mount propagation private and loads loop/snapshot/verity/zero dependencies.

The test binary runs --include-ignored --nocapture --test-threads=1 under a
30-second timeout. All seven pass, none ignored. The previously privileged
cases directly exercise Snapshot::create/write/finish and authenticated chain
reconstruction through kernel mappings, plus Scratch::new recovery of an
interrupted created mapping. Existing non-root import/extent/graph/compressed
stream cases also pass on guest tmpfs. Final polling requires no DM mappings or
attached loops. Whole guest lifetime is additionally bounded by the runner.

Evidence: reproduced/scute-root-tests. This closes the skipped-test caveat for
this source revision/guest only. Startup test constructs an interrupted state;
exact death and concurrent recovery have separate snapshot-death evidence.
No claim of malformed exception-record coverage, generic sandbox adoption or
minimum supported kernel follows from these seven tests.
