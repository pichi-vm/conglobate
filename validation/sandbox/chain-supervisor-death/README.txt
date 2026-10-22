Supervisor death with actual carapace storage
============================================

Reproduce:
  python3 validation/sandbox/guest.py chain-supervisor-death \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-CHAIN-SUPERVISOR-DEATH --run

Disposable KVM guest only; no host disks, shares or NIC. The real chain helper
verifies locked Git/image inputs, assembles the base and writable snapshot,
then launches the mapped FUSE worker and restricted namespace-root command.
The command and a child write bounded heartbeats. The trusted mapped driver
checks both are growing before signalling readiness to the guardian.

A trusted guardian outside the build cgroup stops the supervisor before its
first fork, assigns it to a new cgroup, pins a pidfd and resumes it. The
supervisor runs the chain helper in a private mount namespace. Once both
writers are active, the guardian captures the device names and FUSE abort
handle, kills the supervisor via pidfd, and observes its death. It then kills
the cgroup, aborts the FUSE connection without waiting for clients, waits for
populated=0, and reaps all adopted descendants. These stages are bounded.

Current shared-consumer variant: before terminating the supervisor, the
trusted guardian independently opens read-only handles to the three base
carapace mappings (snapshot, verity and zero). It captures each device identity
and first 4096 bytes. After the failed build's processes and FUSE connection
disappear, only those independently held mappings remain; their identities and
bytes are unchanged and loop backing remains present. Closing the consumer
handles allows the entire DM/loop stack to disappear automatically within the
bounded wait. No dmsetup remove is issued. Base SHA256 is unchanged and no
output is sealed or published.

The preceding v3 variant without independent consumers also passed automatic
teardown. The pinned carapace implementation schedules deferred removal before
returning each mapping. These results validate retained open-handle lifetime
across failure; the guardian's read handles are not a second complete build.

Correction to the initial v2 experiment: that fixture immediately removed the
two remaining names and therefore did NOT establish that explicit device
recovery was necessary. v3 replaces that action with bounded waiting and
passes. The earlier immediate observation was a transient state, not evidence
of a persistent leak. Recovery must not infer ownership from surviving names.

Scope and limits:
- The VM initially has no DM/loop devices. This proves one isolated build's
  recovery, not concurrent ownership safety. CARAPACE-z is a shared name;
  this fixture never removes it explicitly. Captured names alone would not
  establish ownership or protect against identity reuse.
- The guardian remains alive. Its death, machine failure, recovery after restart,
  pre-registration failures and all other launch/teardown boundaries remain open.
- This terminates the worker rather than trusting it to finish an interrupted
  build; nothing may be sealed or published. Successful worker flushing is a
  separate path. Cyclic callbacks and unkillable kernel tasks are not tested.
- Python subreaper instrumentation is test-only, not an exception to the
  production safe-Rust requirement.
- Remaining empty scratch directories and restart scavenging are not asserted;
  the helper's private mount namespace disappears with the cgroup processes.

Current evidence: reproduced/chain-supervisor-shared-consumer. The preceding
no-additional-consumer variant is in reproduced/chain-supervisor-death-deferred. The original
v2 immediate-removal experiment remains in reproduced/chain-supervisor-death. The initial v1 harness treated
DM's textual 'No devices found' output as names and stopped before building;
its log/source catalog is retained in reproduced/chain-supervisor-death-harness-negative.
