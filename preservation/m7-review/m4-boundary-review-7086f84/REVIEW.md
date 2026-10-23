# M4 command-boundary review, baseline 7086f84

Status: partial self-review with an evidence finding. **Not a clean adherence
pass.** No new production defect established in this subset. Full M4 remains
0/3 and its ownership/storage/lifecycle review must continue.

## Source observations

- SANDBOX-DESIGN 4.6: Role::Root constructs the restricted command before
  AssignedRoots::verify_labels. The label probe opens anonymous inodes in /
  and, for bootstrap, confined /mnt, writes/reads/removes security.selinux,
  closes probe handles before command.start. Admission is behavioral; no
  policy loader, registration or revocation participates.
- 7.2–7.3: CommandNamespaces clears supplementary groups before joining the
  mapped user namespace, sets UID/GID zero, creates mount/PID/IPC/UTS plus an
  offline network or joins the supplied distinct network before surrendering
  parent authority. Namespace FDs close before image execution. Creation and
  irreversible setup are documented single-threaded/exit-only operations.
- CommandPrivileges queries supported bounding bits, removes every unallowed
  bit before surrendering SETPCAP, clears ambient/inheritable capabilities,
  sets no_new_privs and checks effective/permitted/bounding/ambient results.
  SYS_ADMIN is absent from the filesystem allowlist.
- CommandSyscalls defaults to EPERM, has architecture guarding, masks namespace
  clone flags and supplies ENOSYS for clone3 fallback. Keyctl/add_key/request_key,
  ptrace/process_vm, unshare/setns and mount-family calls are absent. Socket
  domains, prctl and ioctl arguments have explicit restrictions; mknod variants
  admit FIFO only. This is source evidence, not workload compatibility proof.
- 7.4: runtime targets resolve through root-confined openat2, reject aliasing
  and nesting, and are pinned before mounting. Proc controls are overmounted
  read-only; sys is empty read-only tmpfs. Device inodes are freshly created in
  private tmpfs, with separate devpts/shm. setsid precedes device exposure.
- Root entry uses descriptor-based mount cloning and pivot_root, detaches the
  old root, closes root handles, then restricts credentials and syscalls.
  Resident init is non-dumpable and final descriptor audit rejects live FDs
  above stdio after closing the gate. Stdout/stderr are receiver-relayed pipes.
- 10.3: NamespaceInit blocks in __WALL waitpid and exits on shell completion;
  enclosing containment still owns cancellation and descendant drain. Receiver
  retains its admitted channel during start-gate failure cleanup. These match
  the preserved gate-failure/mutation and descendant VM evidence.

## Evidence finding M4-E1

The maintained tests/distro/confinement/recipe.yaml checks all five capability
sets, no_new_privs, Seccomp=2, mapped IDs, descriptor/proc access, runtime
mounts/devices and per-step recreation. It does **not** attempt restricted
syscalls. Seccomp=2 does not demonstrate that the intended filter denies a
particular operation. A search of current maintained tests found no keyctl,
ptrace or namespace syscall denial regression. Historical experiments are not
a replacement for current composed-command tests.

Required follow-up: a safe Rust fixture executed by an actual recipe command,
with benign owned/no-op operations checking namespace, keyring and process
inspection denial and permitted process/file operations. Exercise the normal
compiled profile, not a test-only policy. Keep syscall arguments innocuous and
all execution inside disposable VMs. Arm profile behavior remains separately
outstanding. Do not assert security from absence of visible host effects.

## Scope and evidence limits

Consulted preserved confinement-v3, descendants-v5 and label-operations-v1
assessments. They are ordinary native amd64 VM evidence at their recorded
source revisions, explicitly not full sandbox qualification. No new VM run
was performed for this review. The 7086f84 full rootless checks are unchanged.
No source edits, new dependencies, scope relaxation, or reference removal.
Hashes identify inspected inputs; they are not a complete M4 scope manifest.
Remaining review includes workspace/cache-backed storage, preparation rollback,
containment/publication records, all deadline/error transitions and evidence
coverage across the complete integrated API.
