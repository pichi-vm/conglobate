Guardian death during an authenticated storage-chain build
===========================================================

Reproduce:
  python3 validation/sandbox/guest.py chain-guardian-death \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-GUARDIAN-DEATH --run

Observed limitation: stopping the one-way cleanup guardian does not stop the
build's two bounded synthetic writers. The fixture uses the real committed/
locked Chain/Snapshot assembly and mapped FUSE worker. An initial trusted gate
assigns the supervisor to its private cgroup before it can fork. Commands enter
the existing restricted namespaces and write only their own fixture files.

The trusted driver records both heartbeat sizes outside the command root. The
independent observer pins the guardian pidfd, waits for both writers to be live,
then kills only that owned guardian. Both heartbeat sizes subsequently increase
and the build cgroup remains populated. The guardian's ordinary finally cleanup
cannot run after SIGKILL. The observer supplies no supervisor-side guardian
monitor, faithfully testing the one-way ownership arrangement rather than a
new recovery mechanism. No production executor is modified.

The outer observer is a test harness owner, not part of the proposed protocol.
It already holds the FUSE abort handle, kills the fixture cgroup, aborts pending
FUSE requests before reaping descendants and removes the empty cgroup. The init
then requires unchanged base SHA256, absent verified/published outputs and no
remaining FUSE connections, DM devices or loop devices. This bounds the test;
it does not establish candidate handling of guardian failure. The host verifier
rejects sealing/reconstruction/publication markers. The fixture's command driver
never requests normal completion, so no claim about eventual publication after
unobserved guardian death is inferred from this test.

All objects exist in a disposable guest without host shares, disks or NICs.
Writer loops and driver waits are finite; the readiness wait is60 seconds and
post-death progress wait3 seconds. Cleanup has bounded reap/wait loops and the
outer VM timeout. Only an explicitly owned guardian is signaled; there is no
exploit or attempt to access a real service.

Evidence: reproduced/chain-guardian-death. Passing harness assertions include
the explicit _LIMITATION marker. This is a demonstrated missing failure policy,
not a passing guardian-loss containment result. A selected executor must define
who owns cleanup after guardian loss, how the remaining supervisor detects it,
and whether the build is failed or transferred to another trusted owner before
publication. That protocol and its registration/late-failure races are still
unimplemented and unvalidated. Supervisor-death cleanup evidence cannot answer
the inverse guardian-death question.
