Normal full build through safe-Rust ownership handoff
=====================================================
Reproduce:
  python3 validation/sandbox/guest.py chain-rust-complete --kernel-version 7.2.10-200.fc44.x86_64 --worker-build /tmp/conglobate-finalize-workers --out /tmp/NEW-rust-complete --run

Uses the exact ownership-init Rust binary source and dedicated Python launcher
from chain-launcher-rust. The exec target /guardian.py is the normal-completion
PID init from chain-pid-complete/owner.py for this fixture. It directly waits for
the actual build helper, inspects/reaps adopted zombies and requires no live or
waitable descendants before successful exit. No child is killed to manufacture
the completion check. The launcher must reap init with status 0.

Observed: restricted command success and descendant termination, sealing,
authenticated restaging, kernel filesystem/content/ownership validation, base
preservation and committed inventory publication. Init reaps one adopted zombie;
no remaining descendants, device-mapper mappings or loop attachments exist at
the respective checks. Guest returns 0. Evidence: reproduced/chain-rust-complete.

Limits: this is the fixed prototype pipeline, not production executor adoption.
The guardian/monitor completion protocol used by failure cases is not exercised
here: this init directly waits for the helper. Output is published before init's
final descendant check, so this is NOT proof of publication eligibility on late
failure or of a transactional commit/ack protocol. The unchanged safe-Rust
ownership boundary permits a successful run; Python still supplies namespace
creation, spawning and trusted init reaping. No hardware attestation is tested.
