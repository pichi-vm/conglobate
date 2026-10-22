Host-backed public DNS and real-VPN prerequisite observation
===========================================================
Observed 2026-10-21 UTC (2026-10-20 America/New_York).
Reproduce the redacted, read-only host summary:
  python3 validation/sandbox/host-dns/profile.py --out /tmp/NEW-profile.json
Reproduce the public DNS probe:
  python3 validation/sandbox/guest.py host-dns-public \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --out /tmp/NEW-host-dns --run
This case intentionally has a QEMU user-network NIC. Unlike the earlier isolated
network fixtures it performs real public DNS queries. It has no host shares or
disks, host port forwards, private-domain queries or application connections.
It changes no host network, DNS, VPN or firewall configuration.

Host observation: /etc/resolv.conf points to systemd-resolved's loopback stub;
NetworkManager reports an active VPN/tun, and resolved reports three search
domains on one link. Only types/counts/loopback classification are retained,
not addresses, connection names or domain strings. This is configuration
observation, not proof that any particular VPN name resolves or is routed.

Guest experiment: QEMU's default user-network gateway/DNS proxy supplies a real
upstream. The guest configures its own veth/NAT/filter, then uses the existing
network-policy launch script and restricted entry helper for the command.
No NET_ADMIN is granted to the command. A public example.com. lookup via libc
succeeds under seccomp/no_new_privs. No returned addresses are printed. The
loopback-stub negative control still fails before injection. Resolver injection
is read-only and original contents are restored for regular and absolute-symlink
layouts. Both layout markers and one GUEST_EXIT0 are checked exactly.

The initial run completed the lookup but expected EROFS from an append open of
a mode0644, initial-root-owned file; mapped root got EACCES first. The retained
negative is a fixture assertion failure, not a DNS failure. Making the owned
injection source mode0666 ensures DAC does not mask the read-only bind check.
The corrected run returns EROFS and passes both layouts.

Scope: establishes host-backed public lookup through this transport plus the
candidate command namespace. It does not prove which host resolver/upstream
answered, VPN-only reachability, search-domain routing, DNSSEC, failover, IPv6
transport, or generic production networking. QEMU user networking is a test
transport, not a selected conglobate or appliance architecture. Existing modeled
inbound/builder filtering tests are separate; they are not reasserted here.

A VPN-only hostname with an expected successful resolution was requested from
the user. That real-VPN check remains pending; do not substitute public lookup
or the presence of an active VPN for that result. Keep the hostname and returned
addresses out of retained artifacts if supplied. Independent goal work remains.
Evidence: reproduced/host-dns-public and host-dns-fixture-negative.
