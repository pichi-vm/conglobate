Complete pipeline restart after active supervisor death
======================================================
Reproduce from repository root:
  python3 validation/sandbox/guest.py chain-restart \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-chain-restart --run

Disposable guest; no host disks/shares/NIC. First attempt uses the existing
supervisor-death guardian around the real authenticated Chain/Snapshot/FUSE
pipeline. The image command makes derived file/metadata changes and runs two
bounded heartbeat writers. Guardian waits for demonstrated writer progress,
pins independent base consumers, kills only the supervisor by pidfd, then kills
the owned cgroup and aborts FUSE before reaping. Independent consumers remain
readable until released. Require no publication/verified output, unchanged base,
and no remaining DM/loop/FUSE resources before restart.

Second attempt uses the same helper binary and byte-identical base, including
the identical /command script. Its trusted orchestrator permits normal command
completion instead of killing the supervisor. A fresh workspace/output avoids
reuse of partially written output or the prior Git working tree. Scratch root
and device namespace remain shared with the first attempt. The command loops
are bounded to 100 iterations (rather than 3000 in the old death-only fixture),
so completion is practical; both attempts execute exactly that same script.

The runner splits separately delimited attempt logs. It requires matching
COMMITTED_LOCKED_CHAIN_INPUTS_PASS lines (Git commit plus OCI index digest), no
seal/restage/publication in the first attempt, and exactly one of each in the
second. Second attempt also requires kernel-remounted data/ownership validation,
unchanged base scute, descendant cleanup, and final inventory publication.
Final checks require unchanged raw base copies and no DM/loop/FUSE resources.

Scope: full fixed pipeline retry from immutable inputs after active-build death
and healthy guardian cleanup. This discards the interrupted writable layer; it
does not resume a partially executed recipe or use its COW as a valid output.
It is not VM reboot/power loss, death during publication, guardian failure,
unknown generic recipes or adoption of the prototype as the production executor.
