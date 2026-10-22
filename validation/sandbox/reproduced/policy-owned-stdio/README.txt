Owned standard streams in the restricted policy fixture
======================================================
Reproduce:
 python3 validation/sandbox/guest.py policy \
  --kernel-version 7.2.10-200.fc44.x86_64 \
  --out /tmp/NEW-policy-owned-stdio --run

Observed: two successive command namespaces pass exact device/inode identity
checks against the trusted parent's prepared descriptors. Stdin is the dedicated
fixture null device, opened read-only; stdout/stderr are the same pipe, opened
write-only. Null reads return EOF. Duplicate output descriptors write distinct
fixed tokens, are closed, and normal logging continues. Each token arrives once.
The collector observes EOF and successful child termination in each step. All
existing policy controls pass; guest exit is exactly zero once.

The driver no longer inherits its own standard streams into these two commands.
It explicitly creates owned pipes/null handles before admission. The collector
limits output to 1 MiB and waits at most ten seconds; it terminates the owned
launch process group on an exceptional collection path. This run exercises
normal EOF, NOT the output-limit/timeout branches, descendant cancellation,
resource exhaustion or a production logging protocol. The credential and
namespace-only control launches are unchanged. Tests use no console operations.

Small-fixture runtime evidence does not qualify every full-build launcher. The
full-chain shell collector and final executor still need their own bounded log
contract. Log markers are diagnostics, never authorization for attestation.
Exact packaging inputs, source snapshots, QEMU command and output are retained.
