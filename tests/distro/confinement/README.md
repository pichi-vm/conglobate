# Command confinement observations

Use this recipe with `tests/distro/run.py --command-probe`, then run `verify.py RUN_DIRECTORY`
(Python cbor2 required). The environment-owned labeling exception is active.

The command steps check:

- PID namespace parentage and open-only denial of trusted PID-1 process views.
- No live inherited descriptors except null input and pipe output/error.
- All five capability sets, `no_new_privs`, seccomp mode, and nonzero outer ID maps.
- Absence of known supervisor/context/cache/backing paths, private device nodes
  and devpts, read-only proc/sysfs views, and loopback-only networking.
- Filesystem persistence alongside fresh `/dev/shm` on the second step.
- EPERM from no-op unshare, a zero-length process_vm_readv targeting self,
  netlink socket creation, and a keyctl read. These operations succeed in an
  unrestricted control process inside the same disposable VM.
- A benign supervisor-owned user-key sentinel is visible in `/proc/keys` before
  and after the build, while the command's `/proc/keys` is empty or inaccessible.
  The sentinel is created only inside the disposable VM and dies with that VM;
  the probe never reads unrelated key payloads or prints key metadata.
- Ordinary Unix sockets and child-process execution remain usable.

The safe Rust probe and host keyctl executable/library runtime are committed
into the fixture context by the runner, then COPY supplies them to the command
root. Commands use the ordinary production filter. No test-only filter or raw
syscall interface is introduced; no process outside the command is targeted.
The fixture removes its copied runtime before output sealing. The VM control
is an availability check, not evidence that credential checks cannot also deny
an operation. `keyctl supports` is deliberately not used as a denial oracle:
it can return success when the capabilities query is denied.

The capability mask is the agreed CHOWN, DAC_OVERRIDE, FOWNER, FSETID, KILL,
SETGID, SETUID, NET_BIND_SERVICE, SYS_CHROOT and SETFCAP profile; SYS_ADMIN is
absent. The test ignores only proc-FD entries already closed after enumeration;
any live descriptor beyond 0/1/2 fails. It does not attempt an escape or prove
complete syscall filtering, ACL/label fidelity, fault handling, cross-architecture
behavior, or confidential-VM qualification. Historical broader tests still need
migration. Failed fixture attempts remain in preservation/m7-review/confinement-v1
and confinement-v2.
