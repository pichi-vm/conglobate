Blocked-worker and group-OOM composition tests
=============================================
Run chain-build-owner-blocked and chain-build-owner-oom:
 python3 validation/sandbox/guest.py CASE \
  --kernel-version 7.2.10-200.fc44.x86_64 \
  --worker-build /tmp/conglobate-finalize-workers --out /tmp/NEW-CASE --run

Both use the actual linked coordinator/arbiter/launcher/namespace-init build
from OWNER-DEATH.txt. The command is active at a bounded trusted rendezvous;
publication remains Building and premature reserve/commit are rejected.

Blocked worker:
The trusted worker records its local daemon PID. The outer observer resolves it
against the exact outer build PID namespace, verifies /patched cmdline and pins
its pidfd. It stops only that owned FUSE daemon, requires stopped state and a
positive connection waiting count before killing only the coordinator. Four
requests were queued in this run. The stopped daemon, arbiter, launcher and init
all terminate; no FUSE/DM/loop resources remain, base bytes are unchanged, and
no completed internal/public artifact appears. No passing-path SIGCONT, FUSE
abort, cgroup kill, init kill or device removal is used. This establishes cleanup
without cooperation from this stopped userspace worker, not arbitrary kernel
uninterruptible I/O or every cyclic filesystem dependency.

Group OOM:
The observer creates one disposable memory cgroup, disables swap and sets
memory.oom.group=1. It starts the coordinator stopped before Python exec, moves
it into that cgroup, then resumes it. All later build children inherit membership.
At the active gate, the coordinator, arbiter, launcher and namespace init must
all appear in cgroup.procs; observer must be outside. Nonzero memory.current is
required. The observer sets memory.max=0 and requires a real increment in both
oom_kill and oom_group_kill plus coordinator SIGKILL status. No synthetic SIGKILL
is sent on the passing path. This run records oom_group_kill=1 and oom_kill=16.

All pinned actors become terminal, FUSE/DM/loop resources disappear, base bytes
remain unchanged and output stays absent. The pressure cgroup must have no
processes and populated0 before its empty directory is removed. Child ownership
links and group-OOM semantics can both contribute to termination; the test does
not attribute every individual death to one mechanism. The observer reaps its
coordinator child; terminal descendant pidfds are not a claim of observer reaping.

Both guests exit0 and harness rejects sealing/publication markers. Exact sources,
inputs, QEMU commands and results are retained independently. These are runtime
composition checks, not adoption of cgroups as a binary deployment requirement.
The observer/kernel have available resources; this is NOT system-wide OOM or a
proof of cleanup under arbitrary kernel allocation failure. Memory charges may
predate migration; no exact RSS/accounting claim is made. Commit-phase ownership
loss, generic executor integration, identity reuse and orphan recovery remain
separate. No new waiver of filesystem metadata semantics or attestation claim.
