Pinned carapace activation tests in a disposable guest
=====================================================

Reproduce:
  python3 validation/sandbox/carapace-activation/build.py --out /tmp/NEW-BUILD
  python3 validation/sandbox/guest.py carapace-activation \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --activation-tests-build /tmp/NEW-BUILD --out /tmp/NEW-GUEST --run

The build script discovers the exact carapace package selected by conglobate's
locked Cargo metadata, copies its source, records hashes, and builds its
existing library tests with loop,image and no default features. This run used
carapace 99caf7941ac8930a5761f6f843b6d8e53dd89587 and devmap 2679426f.
The standalone test build uses carapace's own committed Cargo.lock, including
its test dependencies; it is not the production conglobate dependency graph.
The guest verifies the test executable against the build catalog before use.

Only the four assemble::tests::kernel_ tests run. CARAPACE_REQUIRE_DM_TESTS=1
turns unavailable DM access into failure rather than a skip. Guest timeout is
30 seconds; the harness requires exactly four passing tests and an empty DM
inventory after deferred cleanup. No host shares, disks or NIC are exposed.

Passing checks:
- publication lock is held through creation/resume; injected cooperative error
  rolls back the new target before the lock becomes available;
- competing thread cannot finish activation while publication owns the lock;
  successful publication shares identity, rollback leaves no mapping;
- reuse releases the lock only after pinning the mapping, preserving it when
  the original consumer closes;
- removing the last holder between lookup and open triggers retry/create,
  without treating the vanished device as a valid existing mapping.

Existing dependency tests were run unchanged. These are bounded zero-target
kernel tests with a competing thread, not two complete builds. Cooperative
error injection executes Rust cleanup; it does not prove cleanup after SIGKILL
before deferred removal. Power loss, arbitrary mapping replacement, full
carapace/COW concurrency and shared-cache provenance remain separate gates.

Evidence: reproduced/carapace-activation includes guest source catalog, output,
command, kernel/tool hashes, exact dependency build inputs/log and dependency
source snapshot (without compiled target artifacts).
