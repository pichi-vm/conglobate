Command-local descriptor transfer and trusted pathname socket review
===================================================================
Reproduce:
 python3 validation/sandbox/guest.py policy \
  --kernel-version 7.2.10-200.fc44.x86_64 \
  --out /tmp/NEW-policy-local-rights --run

Two restricted steps each exercise SCM_RIGHTS over an owned socketpair and an
owned pathname Unix socket. Only a newly created command-local regular file is
sent. The child closes its inherited source FD before receiving; the received
FD has the original device/inode, CLOEXEC (requested with MSG_CMSG_CLOEXEC), and
expected bytes. Parent observes the shared file offset and unchanged content.
The child acknowledges and exits successfully. Both socket kinds pass twice;
existing policy and owned-standard-stream checks also pass, with GUEST_EXIT 0.
All objects are disposable, entirely inside the command namespace. This is
normal local IPC compatibility, not a transfer across a trusted boundary.

Boundary evidence retained in the same run:
- Namespace-only control can connect to the owned outer pathname socket while
  its path is visible, despite a private network namespace.
- The restricted command cannot see /supervisor.sock after root replacement.
- Inherited supervisor socket and other high FDs are closed at entry.
These are distinct controls. Do not substitute network isolation for path/FD
isolation, or claim that SCM_RIGHTS is denied by the policy.

Source inventory of selected prototypes (not an exhaustive runtime socket scan):
- chain-command launch/worker and restricted-bootstrap launch create no trusted
  Unix socket listener. Command-created endpoints remain command-local unless
  a trusted component deliberately connects to them.
- publication-protocol/arbitration.py creates /work/arbitrate-CASE with mode0700
  and launches commit-arbiter there. The arbiter binds control.sock in that
  directory. Its RPC assumes a trusted caller; it is not a recipe-facing API.
- That arbitration fixture runs after the build's owner-status check. It does
  not demonstrate live-command exclusion from an integrated coordinator socket.
- The command launchers expose /work/view as root (or as /output under /work/tool),
  not the outer /work directory containing arbitrate-CASE. Preserve that path
  separation and trusted ownership in any composed coordinator.

Remaining: validate the actual coordinator's trusted-socket placement while a
command is live, plus its lifetime/ownership contract. No generic peer-auth
scheme, new syscall restriction or production change was introduced. No claim
about all proc/anonymous-FD interfaces follows from these two transfer tests.
