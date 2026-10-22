Cooperative staging lease validation — 2026-10-21
================================================
Run: python3 validation/sandbox/publication-protocol/recovery/lease.py
Evidence: validation/sandbox/reproduced/recovery-lease.

Six bounded host tmpfs cases exercise actual flock/open-file-description and
process lifetime behavior. Only newly created owned /dev/shm files and child
processes are involved. A diagnostic ctypes prctl makes the harness a subreaper
so the deliberately orphaned descendant is reaped. Every process has a 25s
alarm; protocol readiness and subprocess waits have 5s bounds. No root, network,
public artifact, external process signaling or production change is required.

Observed:
1. An independent open cannot acquire a live owner's exclusive lock.
2. NEGATIVE: LOCK_UN through dup releases the lock while another descriptor
   remains open. Inheritance is not sufficient if any holder explicitly unlocks.
3. NEGATIVE: unlink/recreate allows both old and new inode locks concurrently.
4. After the owner exits, a surviving descendant retaining the shared open-file
   description keeps an independently opened recovery attempt excluded.
5. After all holders exit, a fresh recovery attempt acquires the lock.
6. While one recovery process holds that lock, another process is excluded.
   The stable lease inode and owned staging sentinel remain unchanged.

Design consequence, not an adopted implementation:
A protected stable lease inode and trusted ownership of every shared lock FD
are prerequisites. Never pass the lease FD to untrusted recipe code: it could
unlock the shared description. Never unlink/recreate an active lock pathname.
A trusted keeper could retain it until namespace descendants/writers are gone,
but keeper death must also close writer admission and terminate/reap writers;
this test does not establish that integrated lifetime relationship. Existing
namespace teardown tests are separate evidence, not an automatic composition
proof. Close-on-exec prevents accidental propagation unless explicitly selected.

No staging is garbage-collected by this probe. Acquiring an advisory lock does
not prove all writers participated, completion eligibility, record authenticity,
commit identity, or whether an output was renamed. This is a local Linux tmpfs
process-lifetime result; no reboot/power-loss persistence, NFS or hostile-parent
claim. Persistent protected receipts and restart outcome reconciliation remain
open. The two negative cases are expected limitations, not successful safeguards.
