# External networking implementation batch

Enabled plans require --network-namespace and --resolv-conf. CapturedBuild owns
one immutable NetworkInput with the pinned namespace handle and resolver bytes.
Each CommandNetwork duplicates the pinned handle, stages a read-only resolver
source and uses the existing restricted descriptor-transfer path. Trusted entry
uses explicit CLONE_NEWNET before user-namespace entry; image commands never
receive the handle. Offline steps still create disconnected namespaces.

Removed binary veth creation/configuration, topology allocation/leases, NAT and
firewall installation/recovery, enabled-namespace creator/configuration helper
roles, and netlink-packet-netfilter. The route socket code now serves only offline
loopback setup. Network setup failure has no external network resources/processes
to recover and uses existing blocked-command rejection. Network finalization
retains resolver-source cleanup ownership/deadlines; it cannot remove external
interfaces or policy. Captured resolver contents remain absent from inventory.

Removed source is preserved in previous-network-source.tar.gz at e249a90, with
previous-lifecycle-network.rs and previous-route.rs covering retired probes and
route-dump support. Earlier complete VM source archives and Git history remain.
No legacy/exploratory evidence was removed. Existing untracked review streams
were not staged or modified.

Validation:
- Final root cargo test --all-targets: 220 passing tests (141 library).
- Strict root all-target Clippy passed. Lifecycle fixture compiled and ran.
- Namespace-input VM v1/v2 failures were missing ip and /var/run fixture setup.
  v3 exposed a real mistaken namespace-type preflight. All failures are retained.
  v4 passed pinned-handle replacement and non-destructive descriptor release;
  150/150 archived source comparisons matched.
- DNS VM v1 passed restricted image DNS/TCP, read-only resolver, restored resolver
  symlink/content, following offline step, output export, and retained environment
  namespace/address/firewall table. 163/164 source comparisons matched; the sole
  difference is the subsequently added wrong-type switch in tests/distro/run.py.
- Wrong-type VM v1 returned expected build exit 1. Explicit setns rejected a user
  namespace before command admission; output was absent and cleanup reported
  complete. All 165 compared files matched. This is not independent enumeration
  of every kernel resource.
- Logs, source archives, assessments and local large-asset hashes are adjacent.
  Large assets are local only, not durably archived remotely. DNS test covers a
  controlled reachable VM service; it does not prove external NAT, IPv6, ingress
  denial, confidential platforms, or complete distro boots.

The architecture gate is reopened because the initial three self-review passes
missed bind-mounted namespace FD-link behavior. Corrected contract checks nsfs
and distinct identity at capture, enforcing type at trusted setns. Initial-user-
namespace ownership, NAT/ingress and build-exclusive use remain trusted deployment
requirements. M2 and affected implementation gates need complete adherence
reviews; these tests are not adherence passes. Remaining enabled timeout/nonzero,
shared-namespace step reuse, ingress/IPv6 and broader qualification stay open.
