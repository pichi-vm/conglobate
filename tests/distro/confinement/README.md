# Command confinement observations

Use this recipe with `tests/distro/run.py`, then run `verify.py RUN_DIRECTORY`
(Python cbor2 required). The environment-owned labeling exception is active.

The two command steps check:

- PID namespace parentage and open-only denial of trusted PID-1 process views.
- No live inherited descriptors except null input and pipe output/error.
- All five capability sets, `no_new_privs`, seccomp mode, and nonzero outer ID maps.
- Absence of known supervisor/context/cache/backing paths, private device nodes
  and devpts, read-only proc/sysfs views, and loopback-only networking.
- Filesystem persistence alongside fresh `/dev/shm` on the second step.

The capability mask is the agreed CHOWN, DAC_OVERRIDE, FOWNER, FSETID, KILL,
SETGID, SETUID, NET_BIND_SERVICE, SYS_CHROOT and SETFCAP profile; SYS_ADMIN is
absent. The test ignores only proc-FD entries already closed after enumeration;
any live descriptor beyond 0/1/2 fails. It does not attempt an escape or prove
complete syscall filtering, ACL/label fidelity, fault handling, cross-architecture
behavior, or confidential-VM qualification. Historical broader tests still need
migration. Failed fixture attempts remain in preservation/m7-review/confinement-v1
and confinement-v2.
