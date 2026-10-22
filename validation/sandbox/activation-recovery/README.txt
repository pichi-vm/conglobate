Interrupted snapshot activation recovery states
===============================================

Reproduce:
  python3 validation/sandbox/guest.py activation-recovery \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --out /tmp/NEW-ACTIVATION-RECOVERY --run

Disposable guest, no host shares/disks/NIC. A tiny safe-Rust helper calls the
production Scratch::new entry point under a private TMPDIR. The Python driver
creates bounded DM/lease states corresponding to interrupted activation:
- root-owned empty 0700 lease + created mapping with no table;
- root-owned empty 0700 lease + active zero target without deferred removal.
Startup recovery removes each mapping and its dead lease directory.

Negative controls retain their similarly named mappings and directories:
- exclusive lifetime lock still held by another process;
- nonempty directory (sentinel bytes preserved);
- mode 0755 rather than 0700;
- symlink instead of a directory;
- directory owned by UID/GID 1234 rather than root.
After the live lock is released, the next recovery removes that lease/mapping.
Fixture-owned negative-control mappings are explicitly removed only at the end.

A final active target has deferred removal installed while an independent
read-only FD retains it. Startup collects the empty dead lease directory but
the mapping remains readable; closing the FD permits automatic teardown.
The matrix ends with no DM devices. All recovery calls have five-second process
timeouts; asynchronous removal has a three-second polling deadline.

Original evidence: reproduced/activation-recovery; extended matrix: reproduced/cow-recovery. Production source and helper snapshots,
kernel/tool hashes, command and output accompany the result.

Limits: constructed persistent states, not process-death injection at exact
instructions of Snapshot::create. This exercises lease matching and recovery guards,
not carapace's separately named activation recovery. Simultaneous directory
replacement, device identity reuse, recovery crashes, full build restart and
hostile access to the trusted parent directory remain unqualified. The private
VM and TMPDIR avoid any interaction with real host devices or scratch leases.

Real COW dependency extension
-----------------------------
The current matrix also constructs two real writable persistent snapshots, each
with an 8MiB zero origin and a separate 4MiB loop-backed COW file. It writes and
fsyncs one 4096-byte canary through the snapshot, checks origin bytes stay zero,
and hands dependency lifetime to deferred removal/autoclear. The snapshot itself
is deliberately not deferred, modeling interrupted creator setup.

Production Scratch::new recovers the empty root-owned dead lease. Without a
holder the snapshot, origin and loop attachment disappear. With a held writable
snapshot FD, recovery collects the lease but preserves readable/writable access;
a second canary is written/fsynced before closing that FD. Dependencies then
disappear. A fresh fixture mapping reopens each retained COW and verifies both
the original exception and the expected second block, then explicitly cleans
up only its own recreated resources. All previous preservation controls pass.

This validates real COW dependency teardown and persistent exception data for
constructed interruption states. It is not a recipe/filesystem crash-consistency
claim, exact-instruction process-death injection, or full build restart test.
