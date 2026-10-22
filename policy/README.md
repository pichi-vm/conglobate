# Scoped SELinux label policy

`labels.bpf.c` is the kernel policy asset for the rewrite. It adapts the preserved
kernel-mount validation policy to an explicit set of assigned filesystems. It is
integrated with a safe Rust loader and disabled hook/pin ownership. Recovery
anchors, activation and command admission remain unimplemented.

The `config` array has one native-endian u32 key (zero) and an eight-byte value:
command user-namespace inode number, then mapped root UID. A zero namespace
keeps the exception disabled. The `targets` hash uses an eight-byte key:
superblock owner namespace, then kernel s_dev (major shifted 20 bits plus minor).
Its u32 value must equal one. Capacity two covers the current maximum: bootstrap's
tool root and output root. No owner-only wildcard exists. `src/policy.rs` describes
and encodes this ABI without unsafe layout casts; those descriptions confer no
authority and do not pin reusable identities.

Only the exact, NUL-terminated `security.selinux` name is eligible. The skipcap
hook checks the command namespace and mapped root identity. Set/remove guards
require membership in the assigned target set. Every hook preserves prior LSM
results. Ordinary ownership, ACL, mount and filesystem checks remain active.

The activation owner must keep config disabled while it populates/pins targets and
attaches/pins all three hooks. It enables config last, only with independent
namespace/mount anchors and containment registered. Neither map may change while
commands execute. After independently proving descendant quiescence, disable
config and remove the exception before guards, maps and identity anchors. Failed
cleanup must preserve recovery authority. See SANDBOX-DESIGN.txt and
ARCHITECTURE.md; this asset does not implement those lifecycle guarantees.

The current `LoadedDisabled -> PinnedDisabled` transition pins both maps, then
attaches and pins the set/remove guards before skipcap. Its private root-owned
bpffs directory is accessed through retained descriptors. Setup errors return
partial ownership; cleanup errors retain that ownership for retry. Drop attempts
to remove only the known disabled pins and an empty directory, never recursively
deleting unknown contents. This cleanup strategy is valid only while config stays
zero. An enabled-policy owner must instead preserve independently anchored
identities and pins until descendant quiescence. A directory FD is not such an
anchor and does not establish a persistent mount namespace across process death.

The build compiles and embeds this source through libbpf-cargo 0.27.2, without
generating a Rust skeleton. Runtime loading uses libbpf-rs 0.27.2 and its safe
object/map APIs. Both libraries offer a BSD-2-Clause license alternative;
libbpf-sys 1.7.0 uses BSD-2-Clause. Application Rust and the build script forbid
unsafe code. Build prerequisites include clang with BPF support, a C compiler,
make, pkg-config and development headers/libraries for libelf and zlib. libbpf
is built from the dependency's bundled source; no host package installation is
performed by the build script. The resulting embedded object is the only
production object input; there is no recipe-supplied object path.

The policy retains the prototype's BSD-2-Clause OR GPL-2.0-only license and
GPL-compatible kernel declaration. Rust application code remains Apache-2.0 and
forbids unsafe code. The validation C loader is not a production dependency.

`validation/rewrite-label-policy` records disposable-VM single-target and
two-target tests. `validation/rewrite-policy-loader` tests production disabled
loading, descriptor release and permission failure. Host policy loading is
neither needed nor performed. Broader
lifecycle, concurrency, Arm and confidential-hardware qualification is pending.
