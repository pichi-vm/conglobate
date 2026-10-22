Split-DNS routing contract in a restricted command namespace
============================================================

Reproduce:
  python3 validation/sandbox/guest.py split-dns \
    --kernel-version 7.2.10-200.fc44.x86_64 --out /tmp/NEW-SPLIT-DNS --run

Disposable guest only, no host disks/shares/NIC. Reuses the network-policy
namespace/NAT/filter and restricted safe-Rust entry helper. Commands use a
known regular-file resolver bind mounted read-only; image bytes are unchanged
when each command namespace exits. This fixture does not add a builder-local
DNS exception. All three DNS endpoints live in the external upstream namespace.

Two fixture authorities are reachable at private IPv4 addresses. One answers
mirror.build.test; the other answers mirror.private.test. Each returns NXDOMAIN
for the other's domain. Direct UDP queries establish that both servers work.
With the two addresses flattened into resolv.conf, glibc resolves the first
server's domain and reports EAI_NONAME for the second server's domain. Reversing
the list reverses the failure. This is a DNS routing limitation, not loss of
network reachability or a fallback-on-timeout test.

A third resolver forwards queries to the selected authority according to the
domain suffix. Logs show distinct upstream destinations. With this resolver as
the sole nameserver, restricted commands resolve both names using normal libc
UDP resolution and explicit TCP DNS queries. All addresses are within the
isolated test topology. The simple fixture servers have a 60-second lifetime;
individual sockets have bounded timeouts where they wait for a response.

The positive result demonstrates the required externally reachable resolver
behavior. It does not implement or select a production forwarding service,
configure pichi or a build appliance, or validate real systemd-resolved/VPN
integration. It also does not cover DNSSEC, encrypted DNS, domain search routing,
changing interfaces, resolver failover, malformed messages or arbitrary image
symlink handling. The fixture DNS parser is for fixed test queries only.

Evidence: reproduced/split-dns contains source snapshots, kernel/tool hashes,
command and results. Existing network-policy evidence separately covers builder
endpoint and unsolicited-inbound denial; these are not reasserted by this test.
