Deployment prerequisites for the tested candidate
=================================================

Reproduce:
  python3 validation/sandbox/guest.py deployment \
    --kernel-version 7.2.10-200.fc44.x86_64 --out /tmp/NEW-DEPLOYMENT --run

This is an environment qualification probe, not a new conglobate provisioning
interface. It changes namespace quota and device mode only inside a disposable
guest with no host shares, disks or NICs. Values are restored. A30-second outer
timeout and5-second subprocess limits bound the probe; on timeout the corrected
runner kills/reaps its owned subprocess group. No OS image command is run.

Prerequisite                         Evidence / scope
Trusted initial-root setup           All chain and namespace fixtures use it.
                                     This is not a minimal-capability profile.
Namespace kernel features            USER/PID/NET/IPC/UTS and NAMESPACES config;
                                     actual full UID/GID-map control succeeds.
Namespace quota                      Zero max_user_namespaces rejects simple
                                     creation; restoring it restores mapping.
Parent mapping authority             Dropping SETUID/SETGID from bounding set
                                     rejects the requested 65536-ID mapping.
Reserved ID range                     Fixture maps0..65535 to100000..165535;
                                     non-overlap/lifetime allocation remains
                                     a deployment/implementation requirement.
FUSE device and filesystem            Config/module and root open work; UID
                                     100000 cannot open root-only /dev/fuse,
                                     but can when fixture mode permits it.
FUSE control path                     fusectl mounts and no connections leak;
                                     abort behavior is separately demonstrated
                                     by pressure/chain cancellation fixtures.
Loop and DM storage                  Control devices open; loop/snapshot/verity/
                                     zero modules or built-ins are present.
                                     Actual I/O is covered by chain fixtures.
tmpfs metadata                       TMPFS, XATTR and POSIX_ACL configured;
                                     actual metadata covered by copy-root-tests.
Command restriction support          SECCOMP/FILTER configured; actual helper
                                     behavior covered by policy fixture.
Cleanup/resource environment          cgroup2 mounts, memory/pids controllers
                                     present. Existing guardian/OOM fixtures
                                     exercise delegation and enforcement.
Enabled-network configuration        VETH/NF_TABLES/NF_NAT configured and
                                     NAT/conntrack modules load; network-policy
                                     separately qualifies modeled traffic.
Path/process/publication APIs         Existing resolver-target, process-policy
                                     and artifact tests exercise openat2,
                                     pidfds and NOREPLACE publication. This
                                     new probe does not retest those APIs.
Userspace trust inputs               Exact guest binaries/libraries/kernel
                                     are hashed; full trusted-runtime binding
                                     and native maintenance remain open.

The kernel config copied from /boot is hashed alongside the corresponding
guest vmlinuz. The successful baseline is Fedora44 x86_64 kernel7.2.10-200.fc44,
not a claim that another configuration with matching config symbols works.
No minimum kernel version or portability guarantee is inferred. Kernel config
availability alone is not a behavioral test. Runtime operation controls and
the referenced existing fixtures supply narrower evidence for each component.

The FUSE mode0666 is only the positive test control, not a recommendation to
expose it to commands. The chosen deployment must grant access only where
needed by trusted setup/worker paths. The worker/backend also needs its assigned
writable snapshot and trusted executable/library/configuration inputs. Command
roots must not receive those devices or control paths. Commands remain mapped
namespace root; these results do not show fully rootless storage assembly.

Initial failure: the full unshare command with mapping coordination timed out
when max_user_namespaces was zero. The5-second timeout is retained in
reproduced/deployment-mapping-timeout. Its cause has not been source-debugged;
do not label it a kernel deadlock or a conglobate defect. A simpler unshare
creation request returns the expected no-space failure. The full mapping path
still succeeds normally and rejects missing mapping authority. The command
wrapper is util-linux2.41.5; the intended binary should use safe library APIs
with explicit failure/cleanup handling rather than rely on this CLI behavior.

Passing evidence: reproduced/deployment. Module/device/config checks do not
replace maintained backend selection or compatibility qualification. The
environment remains responsible for its limits and provisioning, as requested.
Distribution-specific userns restrictions, LSM policy variants, allocation of
non-overlapping concurrent ranges, and a final deployed runtime profile remain
open; no appliance configuration or pichi behavior is specified here.

Primary references checked2026-10-20:
https://docs.kernel.org/admin-guide/sysctl/user.html
https://man7.org/linux/man-pages/man7/user_namespaces.7.html
https://docs.kernel.org/filesystems/fuse/fuse.html
Namespace limits apply per user/user namespace, with ancestor accounting.
Writing UID/GID mappings is subject to parent-namespace permissions. FUSE uses
an opened /dev/fuse descriptor; fusectl provides connection abort controls.
