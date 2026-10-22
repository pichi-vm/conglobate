Overlapping loss of both candidate lifecycle owners
===================================================
Reproduce:
  python3 validation/sandbox/guest.py chain-owners-death \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-owners-death --run

Disposable guest only, no host shares/disks/NIC. Uses the real authenticated
chain/FUSE command and the candidate guardian-watch monitor. After both writers
progress and the monitor arms its abort handle, an independent fixture observer
pins guardian and monitor pidfds. It verifies the monitor's parent identity,
stops the monitor, kills/reaps guardian, then kills monitor and confirms pidfd
readiness. The stop makes loss deterministic: monitor cannot process guardian
notification between the two deaths. This is one ordered overlapping failure,
not an assertion that two signals were delivered at exactly the same instant.

The probe checks for continued progress of both fixture writers and a populated
build cgroup, while the monitor completion marker is absent. A success marker
here means a known LIMITATION was reproduced. The harness then kills the build
cgroup and aborts FUSE, reaps descendants, removes the group and requires no
verified/published output. Init checks unchanged base and final DM/loop/FUSE
cleanup. That cleanup belongs to the external test owner, not the dead candidate
owners, and cannot establish fail-closed behavior of the proposed arrangement.

All activity is bounded by the existing per-condition/reap deadlines and guest
timeout. Only the fixture's known processes are signaled; no commands or control
objects from the host are exposed. Harness rejects any monitor cleanup success
or sealing/publication marker in this case.

Design implication if reproduced: mutual pidfd watches require at least one
live owner; they do not establish a kernel lifetime boundary for a build after
both owners disappear. Whole-build PID namespace init ownership or an external
appliance lifetime boundary needs separate design/validation. No such repair is
implemented by this test. Publication-time eligibility remains a separate gap.

Observed result: both writers continue after both owners die. Known limitation
confirmed; external harness cleanup passes. Evidence: reproduced/chain-owners-death.
