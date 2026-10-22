Resolver setup interruption through the authenticated FUSE chain
===============================================================

Run each stage in a fresh output directory:
  python3 validation/sandbox/guest.py chain-resolver-cancel \
    --resolver-cancel-stage before-bind \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-RESOLVER-CANCEL --run
Repeat with after-bind and after-readonly. The latter two stop after the first
target's bind/remount, while the second target is still pending. These are
three defined boundaries, not all possible instruction-level interruptions.

The actual Chain/Snapshot pipeline mounts the experimental finalization-patched
FUSE worker. Trusted setup selects image resolver paths through the safe Rust
helper. A test-only RESOLVER_TEST_GATE holds that helper, which is PID1 of the
command namespace, for at most10 seconds at the selected boundary. An atomic
marker gives its ancestor-visible PID to the trusted controller. The controller
opens a pidfd while the helper is gated and cancels only that owned setup process.
No image command has executed. The controller requires the gate within8 seconds
and process termination within5 seconds; the guest has an independent timeout.

Before and after cancellation, the controller verifies source/image resolver
bytes, inode, mode and ownership and its unchanged mountinfo. It requires the
namespace init to disappear and scans readable process namespace links for any
remaining holder of that mount namespace. Other users' unreadable proc links
are skipped; this is not a claim to inspect every process's namespace FDs.

The controller returns125, the worker wrapper gracefully unmounts/reaps its
FUSE daemon, and the actual chain helper's status assertion rejects the build.
Helper panic101 is the existing prototype failure gate, not a production error
contract. No output/verified image may exist, and the base hash must match.
FUSE connections, DM targets and loop devices must disappear. The host verifier
also rejects any layer-sealed, reconstructed-chain or publication-success marker.
The source and fixture paths are entirely within the disposable guest.

The first attempted before-bind run omitted the helper binary from packaging.
Although ordinary failure cleanup ran, the required cancellation marker was
absent and the harness rejected it. That negative evidence is retained in
reproduced/chain-resolver-cancel-packaging-negative, not counted as a pass.

This covers setup cancellation while the supervisor and filesystem worker
remain alive. It does not qualify a blocked filesystem operation at the gate,
controller/worker death during cancellation, a concurrent image writer, later
trusted continuation boundaries, or real host/VPN DNS policy. Other fixtures
cover narrower worker-hang/death cases independently; do not silently combine
them into evidence for every failure combination. No production executor or
public conglobate interface is changed by these test hooks.
