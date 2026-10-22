Explicit trusted-worker launch environment in the full FUSE build
================================================================
Reproduce:
  python3 validation/sandbox/guest.py chain-launch-env \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-launch-env --run

Two independent builds run in a disposable guest without host disks, shares or
network. Each receives a trusted diagnostic BASH_ENV pointing to one fixed
fixture script. The script only writes an owned marker in /work when invoked
as /chain-worker. It reads no secrets, changes no policy, and is not an image
script. Both image commands remain behind the existing bounded readiness gate.

A uses the previous inherited-environment launcher. B uses the validation-only
CHAIN_SANITIZE_LAUNCH path: absolute /usr/bin/unshare, Command::env_clear and
explicit PATH=/usr/bin:/usr/sbin, HOME=/, LC_ALL=C. Diagnostic custom launcher
selection is rejected for this path; other cases retain their existing behavior.
The initial parent/helper environment is still a trusted fixture assumption.

At the pre-command gate, both workers are ready. A's marker must exist and B's
must not. The same distinction must hold after both full command/seal/kernel
verification/inventory publication paths finish. Two occurrences of all main
build-success markers and no residual loop/DM devices are required. This checks
that constructing the environment works with the actual worker and setup tools,
not only that a small isolated exec fixture drops a variable.

Scope: a bounded negative/control for one setup-shell environment channel.
It does not imply recipe commands can modify the parent's environment or that
this fixture demonstrates initial-root execution. The worker has already entered
its mapped user namespace, but has storage/setup authority unavailable to recipe
commands. Clearing the final image exec environment was too late for this shell.

The change is in the diagnostic helper only. Initial supervisor startup,
LD_PRELOAD/loader configuration, immutable executable/library trees, the complete
launcher/process profile and other production child-spawn paths still require
their own contract and integration. No general environment-security or final
executor qualification is claimed. Earlier loader tests remain separate evidence.
