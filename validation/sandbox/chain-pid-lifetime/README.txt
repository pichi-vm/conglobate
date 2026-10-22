Whole-build PID namespace lifetime candidate
===========================================
Reproduce:
  python3 validation/sandbox/guest.py chain-pid-lifetime \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-outer-pid-lifetime --run

Disposable guest, no host shares/disks/NIC. Existing guardian/monitor/real-chain
fixture is placed inside an additional PID and mount namespace with its own
proc mount. Guardian is PID 1 there; monitor, supervisor, trusted FUSE worker,
restricted command namespace and all command descendants are inside that outer
lifetime namespace. Existing command user/PID/network/mount restrictions remain.
No external runtime is proposed by using util-linux to construct this prototype.

Observer verifies host-visible NSpid identities and parent/child relations,
pins namespace init and monitor pidfds, and requires two live writers plus an
armed monitor/FUSE connection. It stops monitor so user-space cleanup cannot run,
then kills only namespace init. Kernel teardown must terminate descendants even
with the monitor stopped. Observer waits for unshare to return, an empty build
cgroup, no FUSE connections and stable writer progress. It removes only the empty
cgroup directory; no cgroup.kill, monitor signal or FUSE abort occurs on pass.
Failure-only cleanup cannot produce the required success marker.

Result: passing active-build test; unchanged base, no seal/publication, and final
DM/loop/FUSE cleanup. Evidence: reproduced/chain-pid-lifetime. The previous
noncontained two-owner failure remains in reproduced/chain-owners-death, where
writers continued until external cleanup. Source catalogs preserve both setups.

This is a candidate kernel lifetime boundary, not an adopted production design.
It addresses the tested init-death/disabled-monitor ordering. Still qualify
normal completion/publication, ownership handoff before namespace init exists,
loss of the outer launcher without init death, hung filesystem/syscall cases,
resource exhaustion, multibuild operation and safe-Rust integration. The shell/
Python prototypes do not provide a final trusted bootstrap or attestation chain.
The observer's own death and VM power loss are not injected. Command PID isolation
alone was already present in the failed setup; this additional namespace contains
ALL build actors, including the trusted worker and supervisor.

Stopped filesystem worker extension
-----------------------------------
Reproduce with case chain-pid-blocked and the same other runner arguments. The
worker records its namespace-local daemon PID. Observer resolves it against the
build cgroup's host-visible process IDs, verifies its PID namespace matches init,
and pins its pidfd. After live writer progress it stops the daemon and requires
FUSE waiting > 0, then stops monitor and kills only outer namespace init.

Result: two queued requests observed; kernel descendant teardown, FUSE release,
empty cgroup, stable progress, unchanged base and final DM/loop cleanup pass. No
observer FUSE abort or cgroup.kill occurs on pass. Evidence:
reproduced/chain-pid-blocked. This is stopped userspace-worker coverage, not
uninterruptible kernel I/O, every cyclic dependency or system-wide exhaustion.

Outer launcher lifetime link
-----------------------------
Reproduce cases chain-launcher-unlinked and chain-launcher-linked with the same
other runner arguments. Both wait for actual writers, stop the monitor, and kill
only the external unshare launcher through a pinned pidfd.

Unlinked control: guardian pidfd is not ready, the build cgroup remains populated
and both writers progress. Observer then explicitly kills namespace init as
fixture cleanup; that is not automatic launcher-loss handling. Linked variant:
unshare --kill-child=KILL causes teardown after launcher death with no observer
init signal, cgroup kill or FUSE abort. Both end with unchanged base, no output
and no DM/loop/FUSE residue. The linked test validates resulting build teardown;
it does not trace the utility's internal syscalls.

Evidence: reproduced/chain-launcher-unlinked and chain-launcher-linked. Packaged
util-linux is 2.41.5; --help describes --kill-child as killing the forked child
when the launcher dies. This demonstrates a useful lifetime primitive without
proposing an external runtime dependency for conglobate. Final safe-Rust launch
needs equivalent semantics and must address death before link setup/ownership
handoff. Tests inject only after the active build is established.

Safe-Rust ownership setup in the actual build fixture
-----------------------------------------------------
Reproduce case chain-launcher-rust with the same kernel/worker-build arguments.
A dedicated single-threaded Python fixture launcher opens its own pidfd, creates
private mount/future-child PID namespaces, then starts ownership-init with the
pidfd on stdin. This replaces the utility's --kill-child behavior for this case.

ownership-init forbids unsafe Rust, checks that it is PID 1 with outside parent,
arms SIGKILL, polls the original-parent handle, mounts private proc and execs the
existing guardian with /dev/null stdin. No credential transition follows signal
setup. The guardian/monitor, actual supervisor, FUSE worker and command writers
run inside that lifetime namespace. After actual writer progress, observer stops
monitor and kills only the dedicated launcher. It requires init pidfd readiness,
empty cgroup, no FUSE connections, stable writers, unchanged base, no seal/output,
and eventual absence of device-mapper and loop resources. No passing-path init
signal, cgroup.kill or FUSE abort is permitted.

Evidence: reproduced/chain-launcher-rust. This integrates the safe-Rust boundary
helper into the actual fixed build pipeline, not into the production executor.
Namespace creation and process spawning still use a trusted Python fixture;
this is not a wholly Rust launcher implementation or final runtime binding.
Prior primitive handoff tests qualify selected early-death gates; this pipeline
case injects death only after writers become active. Publication/normal shutdown,
full startup fault matrix and resource-exhaustion integration remain separate.

The chain-launcher-rust-blocked variant additionally stops the actual FUSE worker
and requires queued FUSE requests before stopping monitor and killing launcher.
It passes the same no-fallback cleanup checks; two requests were queued in the
retained run (reproduced/chain-launcher-rust-blocked). This qualifies a stopped
userspace server, not uninterruptible kernel I/O or arbitrary dependency cycles.
