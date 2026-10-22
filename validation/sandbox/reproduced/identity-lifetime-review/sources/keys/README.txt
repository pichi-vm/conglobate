Session possession versus worker key ownership
==============================================
Run:
  python3 validation/sandbox/guest.py keys --kernel-version 7.2.10-200.fc44.x86_64 --out /tmp/NEW-KEYS --run
  python3 validation/sandbox/guest.py policy --kernel-version 7.2.10-200.fc44.x86_64 --out /tmp/NEW-KEY-POLICY --run

The original key fixture creates a synthetic supervisor session key. After
user/mount/PID/net/IPC/UTS namespace creation, inherited possession still permits
reading it. Joining an anonymous session removes that tested access.

The extension keeps a worker at host UID100000 alive with one synthetic user
key anchored in its per-UID keyring. It explicitly links that ring to its
private session to obtain normal possession during setup, then sets the key
to owner-only permissions (0x003f0000). A separate command namespace joins a
fresh anonymous session and reads by the known serial number:
- mapped root at host UID100000 can read the worker-owned canary;
- mapped root at host UID200000 is denied.
The worker verifies its key is unchanged, then revokes the disposable key.
The first setup attempt omitted the user-ring/session link and could not set
permissions; correcting possession during fixture setup enabled the test.
No host keyrings or actual credentials are involved: all runs use the guest.

The expanded policy test deliberately retains a readable session canary. Its
namespace-only control reads the real key by serial. After the restricted
entry helper runs, that same valid keyctl READ returns EPERM in two successive
steps. The supervisor can still read its unchanged value and then revokes it.
This complements prior null-argument syscall denials with an otherwise-allowed
operation. The test does not modify production restrictions or adopt a new
runtime policy.

Evidence: ../reproduced/worker-keys and ../reproduced/key-policy, with inputs,
commands, results, and (for policy) helper source/lock hashes.

Implication: separate sessions remove inherited possession, but not permissions
based on matching owner UID. A design sharing worker/command host identities
must not treat session replacement as sufficient key isolation. The candidate
keyctl/add_key/request_key denylist has a passing real-read control; workloads
requiring these APIs still need compatibility assessment. User-key permission
semantics, UID reuse after teardown, persistent keyrings, request-key upcalls,
and other key types are not exhaustively qualified here. No complete keyring
or cross-build isolation claim is made.

Primary reference checked 2026-10-20:
  https://man7.org/linux/man-pages/man7/keyrings.7.html
Keys have separate ownership and possession permissions. Per-UID keyrings and
process/session anchors have distinct sharing/lifetime rules. The observed
same-owner behavior is consistent with these rules, not a kernel defect.
Python ctypes syscall instrumentation remains confined to experimental tests;
production conglobate's safe-Rust requirement is unchanged.

UID reuse after worker exit with a retained FD
----------------------------------------------
keys/reuse.py runs after the original controls. UID300000 worker creates an
owner-only disposable user key and a memfd with a separate known file canary.
It transfers the FD by SCM_RIGHTS to the trusted observer, then exits and is
reaped. With the FD still held, fresh-session mapped root at UID300000 reads
the old key; UID400000 is denied. Trusted UID300000 cleanup revokes that owned
key; another same-UID read gets EKEYREVOKED or ENOKEY. The file canary is unchanged
and the FD is closed. All expected controls pass: ../reproduced/keys-reuse.

Primary reference checked 2026-10-20:
https://man7.org/linux/man-pages/man7/user-keyring.7.html
The documented UID-keyring lifetime includes open files from that UID; links
can also retain keyrings. This experiment establishes readable residue after
process exit with an FD retained, not that this FD is the sole anchor. It does
not measure garbage collection after closure or test persistent keyrings.

Empty process membership alone is insufficient evidence for identity recycling.
Retained file credentials and other anchors require lifetime accounting. Fresh
anonymous sessions do not erase owner access. Revoking this known owned canary
works; this is not authorization to revoke arbitrary real-machine per-UID keys.
Use dedicated identities and tracked resources, retaining command key-syscall
denials. This unfiltered diagnostic tests UID semantics, not a bypass of the
separately validated keyctl/add_key/request_key restrictions. Final allocator
and trusted-worker integration remain open; no general safe-reuse claim is made.
