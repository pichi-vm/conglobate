Enabled networking and resolver mounts under command restrictions
=================================================================

Reproduce:
  python3 validation/sandbox/guest.py network-policy \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --out /tmp/NEW-NETWORK-POLICY-GUEST --run

The entire topology is in a disposable guest with no host shares/disks/NICs:
an upstream namespace, the trusted builder network, and the command namespace.
Private IPv4 and IPv6 upstream addresses are intentionally reachable. NAT and
stateful filtering allow outbound traffic/replies, deny builder endpoints and
unsolicited inbound, and permit IPv6 neighbor discovery on the command link.
The trusted builder configures networking before entering the command's user
namespace; commands receive no NET_ADMIN.

Commands use the safe-Rust entry helper with the current capability/seccomp/
socket policy, mapped root, private mount/PID/IPC/UTS namespaces, pivot/detach,
FD closure and clean environment. Network setup uses the already-created command
namespace. The client asserts no_new_privs and active seccomp. The synthetic
root contains tools and fixture scripts, not a FUSE OS image; this tests network
integration with the candidate entry sequence, not a whole production build.

Verified for regular /etc/resolv.conf and an absolute symlink into /run:
- A 127.0.0.53 image stub, with no resolver in this command namespace, fails
  with EAI_AGAIN before injection. It is not transparently the builder's stub.
- A trusted resolver file bound read-only over the known target enables lookup.
  Attempted modification returns EROFS.
- A short name uses the configured search domain. The fixture DNS server only
  answers mirror.build.test; unknown names return NXDOMAIN, avoiding a wildcard
  server that could conceal broken search behavior.
- UDP DNS, TCP DNS, A/AAAA lookup and HTTP over explicit IPv4 and IPv6 succeed.
- Builder endpoints cannot be reached over either family. A trusted positive
  control first confirms both builder listeners actually respond.
- Restricted command listeners exist for the unsolicited inbound checks; both
  IPv4 and IPv6 connections from upstream fail.
- After step namespace exit, the image's original resolver target bytes remain
  unchanged, its symlink remains, and the builder's resolver file is unchanged.

The mount target is selected from two known fixture paths. This is NOT an
arbitrary image-path resolver or a race-resistant mounting implementation.
Missing/dangling links, permission/ownership variants, hostile paths, changing
resolver configuration and cancellation during injection need additional gates.
The underlying files are stable during trusted setup.

Evidence: ../reproduced/network-policy/{inputs.json,command.json,results.txt}.
Input hashes include the actual entry helper, fixture code and packaged runtime.

Remaining DNS contract:
This does not validate actual VPN/split-DNS integration. systemd-resolved routes
queries using per-link domains and server configuration, which cannot be
represented by merely flattening all server addresses into resolv.conf. The
upstream environment must provide a reachable resolver preserving the intended
resolution behavior, or a separately validated restricted forwarding interface.
No such forwarding implementation or appliance behavior is selected here.
These tests do not grant commands access to the builder's general services.

Primary references:
https://www.freedesktop.org/software/systemd/man/latest/systemd-resolved.service.html
https://man7.org/linux/man-pages/man5/resolv.conf.5.html

The documented local stub is on loopback; resolv.conf supplies server addresses
and search domains. The negative control and read-only mount tests establish the
specific namespace behavior above, not a full resolver compatibility claim.
