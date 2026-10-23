# M4 command-boundary inspection

This is a source/evidence review component, not a complete clean adherence pass.
The M4 streak remains zero. Existing v154 runtime evidence covers amd64 only.

| Requirement | Source inspected | Evidence and limits |
| --- | --- | --- |
| Mapped root, isolated mount/PID/network/IPC/UTS | isolation/namespaces.rs, isolation/init.rs | Entry clears supplementary groups, joins supplied userns, becomes mapped root and unshares the five namespaces. Private propagation precedes trusted PID-1 spawn. Init checks non-host-root mapping. VM fixture checks namespace identities and root mapping; enabled networking remains M6. |
| No SYS_ADMIN in any command set | process/privileges.rs | Immutable allowlist omits SYS_ADMIN, SETPCAP and SYS_PTRACE; supported bounding bits are queried from kernel. Finalization sets no_new_privs, clears ambient/inheritable sets, drops unwanted bounding bits and checks all five sets. VM checks survive image-shell exec. |
| No new namespace/mount authority | process/syscalls.rs | Default EPERM policy; clone namespace mask, clone3 ENOSYS fallback; no unshare/setns/mount/new-mount API allow entries. Socket families and prctl/ioctl values constrained. VM proves ordinary threads work and unshare userns fails. This is not exhaustive runtime coverage of every denied syscall or an arm64 result. |
| No outside-root handles | isolation/root.rs, isolation/receiver.rs | Pivot/detach old root and close root descriptors; nondumpable resident init; descriptor audit rejects surviving FDs above 2. Null stdin and receiver-created diagnostic pipes occupy 0–2. v149 covers supervisor diagnostics redirected to a file; v154 covers blocked relay cancellation. Low-level root API alone does not authenticate stdout/stderr: production composition supplies those pipes. |
| Private runtime mounts | isolation/runtime.rs, isolation/devices.rs | Targets resolved IN_ROOT/NO_MAGICLINKS/NO_XDEV and pinned before mounting; reject aliases/nesting. Fresh proc, readonly empty sys, private device tmpfs/devpts/shm; image remains nodev/nosuid. Fresh device inodes restricted to null/zero/full/random/urandom/tty. No automatic tmp/run mount. VM checks proc controls, readonly sys, device behavior and fresh shm on second command. |
| Explicit admission after restrictions | isolation/setup/gated/helper.rs, isolation/receiver.rs | Root pivot, privileges and seccomp precede ready notification. Receiver reports prepared, awaits supervisor admission, then releases leaf gate. Error paths remain exit-only; parent retains cancellation/storage/policy. Existing pre-admission and failed-setup VM cases assert no command success. |
| Shell contract and descendant lifetime | isolation/root.rs, isolation/init.rs | /bin/sh -eu -c; cwd /; cleared environment with PATH/HOME/LANG. CARAPACE remains explicitly absent until M6. PID 1 waits with __WALL, reports shell outcome by exit; kernel namespace exit plus outer cgroup cancellation/reaping handles survivors. VM checks adopted descendants and nonzero status. |
| Narrow foreign-label authorization | policy/labels.bpf.c, policy/bound.rs | Exact security.selinux name including terminator; exact mapped root/userns; assigned superblock-owner/device membership; prior hook results preserved. Binding checks subject mapping and distinct pinned devices, retains identities through disabled-policy cleanup. Trusted initial-superblock provenance is required; stat cannot prove it. VM checks successful set/remove, prefix rejection, outside-policy tmpfs rejection and remount persistence. |

Rust review: boundary descriptions are immutable; entering/restricting/starting
consume their owners; state traits are sealed where states are generic. Helpers
are deliberately irreversible and exit-only after partial transitions. Root and
namespace types are !Send through an Rc marker; no unsafe application Rust is
introduced. BPF C remains a separate, explicitly scoped kernel policy component.
No new freestanding application functions were introduced.

No contradiction found in this inspected component. Full M4 acceptance still
requires the acquisition/rollback and API review to be consolidated with storage
and lifecycle observations, current scope/evidence checks, and three complete
clean passes. Metadata fidelity across dual-mount bootstrap is M5; distro and
arm64/confidential-platform qualification is M7, not established by this review.
