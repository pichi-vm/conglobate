Whole-build filesystem exhaustion — 2026-10-21 UTC
=================================================
Reproduce:
 python3 validation/sandbox/guest.py chain-filesystem-full \
  --kernel-version 7.2.10-200.fc44.x86_64 \
  --worker-build /tmp/conglobate-finalize-workers --out NEW --run

Owned64MiB ext4 fixture uses4KiB blocks. The existing chain/publication path
imports/authenticates a base, prepares committed Git/locked image inputs, creates
a writable snapshot and executes through the mapped-FUSE worker and restricted
entry helper. The command checks PID1, no-new-privileges, seccomp and absent
supervisor/context/device paths. It performs the existing ordinary file edits,
then at most1024 writes of64KiB to one file using shell printf. No direct block
access, process attack, host share, NIC or injected exit7 is involved.

A real printf write reports ENOSPC and set-e propagates exit1. The worker trap
unmounts/waits; the diagnostic chain helper rejects failed command completion
with exit101. The trusted guest asserts no verified path and no published output,
unchanged base hash, no DM mappings and no attached loops. The runner rejects
any sealing/publication marker or bounded-write-loop completion. Guest exit0
means these expected-failure assertions passed, not that the build succeeded.

This closes the missing ordinary filesystem-full-to-build-rejection check in
areas6/16/17. It differs from previous artifact ENOSPC, quota/OOM and synthetic
command exit tests. It does not cover delayed worker errors after command success,
inode exhaustion, every pipeline stage, simultaneous coordinator death or the
final executor. Those existing findings/gates remain. No production code changed.
The worker is the existing finalize-workers fixture; this is not a new claim
about maintained ACL fixes or all filesystem metadata compatibility.
