Safe-Rust parent-death ownership handoff
=======================================
Run:
  python3 validation/sandbox/guest.py parent-handoff --kernel-version 7.2.10-200.fc44.x86_64 --out /tmp/conglobate-parent-handoff-new --run

Trusted parent opens its own pidfd and passes it as stdin to a safe-Rust child.
The child arms SIGKILL via PR_SET_PDEATHSIG, then polls the original parent pidfd
before admitting work. No recipe commands run in this primitive test.

Observer pins both processes, acts as subreaper and forces death at three gates:
- before arming: resumed child rejects dead original parent with exit 1;
- after arming, before check: child dies with SIGKILL;
- after check: child dies with SIGKILL before finishing.
A live-parent control completes normally. All four repeat with child as PID 1
of a new PID namespace, while parent and observer remain outside it. Child sees
getppid() == 0, and observer verifies NSpid against its pinned host-side process.
The nested init gate uses an ancestor-issued SIGSTOP; self-stop does not work
for this fixture's namespace init. Before-arm death is still rejected after
reparenting to the observer, rather than attaching trust to the new parent.

Evidence:
  reproduced/parent-handoff: original four same-namespace cases pass.
  reproduced/parent-handoff-nested-fixture-negative: self-stop fixture failure.
  reproduced/parent-handoff-nested: eight same/nested cases pass.

Limits:
This does not integrate the full command/worker lifecycle or publication.
The fd is supplied by a trusted launcher; this is not validation of an arbitrary
untrusted descriptor. Stdin must be replaced before command execution. Tests use
the credential/thread/ordinary-exec extensions described below. Final launch
ordering, filesystem credential changes and privileged exec remain open. The
trusted outer namespace init should retain its identity and launch workers
separately. This is candidate evidence, not production security certification.

Credential, exec and thread extension
------------------------------------
Latest retained run: reproduced/parent-handoff-thread (22 expected outcomes).
Intermediate credential-only extension: reproduced/parent-handoff-credentials.

Both PID arrangements now also demonstrate:
- ordinary exec of the same helper preserves the armed SIGKILL (checked before
  any re-arm); subsequent parent death kills it;
- setresgid to 100000 clears the signal; after re-arming, setresuid to 100000
  independently clears it again;
- leaving the signal cleared allows the diagnostic child to survive parent
  death and write its completion marker: a reproduced limitation, NOT isolation;
- re-arm plus original-parent check after those transitions rejects death in
  the unarmed gap, kills after admission, and completes with a live parent;
- creating-thread exit before arm, with process leader kept alive, is NOT
  detected by the process pidfd. The child admits and completes diagnostic work;
- creating-thread exit after arm kills the child even though the process leader
  is alive. The leader reaps it and reports signal status.
The observer checks leader pidfd non-readiness after joining the creating thread,
so the thread result is not inferred from whole-process death. No recipe or
external resource is touched. All children are confined to the disposable guest.

Consequences: use a dedicated persistent owner (preferably a single-threaded
launcher); a process pidfd is not a substitute for owning-thread lifetime.
Re-arm/check after credential changes, or keep outer namespace init credentials
unchanged and launch workers separately. This suite demonstrates primitives and
limitations; it does not validate the full executor, privileged exec, fsuid/fsgid
changes, arbitrary thread-pool launch, or final FD handoff.

Primary reference checked 2026-10-20:
https://man7.org/linux/man-pages/man2/PR_SET_PDEATHSIG.2const.html
Linux man-pages 6.19 documents thread ownership, ordinary exec preservation,
credential-change clearing, and privileged-exec clearing. Tests above validate
the explicitly listed subset; other documented cases are not claimed tested.
