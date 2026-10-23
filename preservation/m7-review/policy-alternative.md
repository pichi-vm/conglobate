# Environment-loaded labeling exception: design discussion

The user identified that a dedicated build VM with SELinux disabled may use
an always-active exception for exactly `security.selinux`. This eliminates the
need for per-command namespace/filesystem registrations. The implementation
still uses scoped registrations; this note records the alternative and does not
claim it is validated or silently supersede the current design.

The exception bypasses the capability gate for this xattr, not all LSM checks.
Linux xattr_permission delegates security.* permission decisions before ordinary
inode_permission. Therefore, ordinary file write-mode denial is NOT sufficient
to protect labels on reachable files. Read-only mounts and other applicable
VFS/LSM/filesystem checks remain relevant. With SELinux disabled, label values
do not grant SELinux authority, but labels remain integrity-sensitive output.

If adopted, the build environment loads the fixed policy once. Conglobate drops
policy map updates, per-step attachment/pinning, policy identity retention,
activation/disable transitions and policy-only recovery states. Process draining,
mount ownership, restricted namespaces, seccomp, capability restrictions and
fallible storage cleanup remain necessary. An environment-owned artifact also
allows removal of the binary's BPF compiler/loader dependencies.

Before adoption, use only disposable-VM benign metadata tests to establish:

1. Label set/read/remove works on assigned writable filesystems with command
   SYS_ADMIN absent and SELinux disabled, including foreign policy labels.
2. The exception matches security.selinux exactly; security.capability and
   unrelated protected attributes receive no additional authorization.
3. Read-only assigned views reject label mutation. No supervisor/cache/context
   writable inode or descriptor is reachable from the command sandbox.
4. A missing or incompatible external policy fails required startup validation;
   successful behavior is not inferred merely from a familiar pinned pathname.
5. Existing teardown, descendant-draining and bootstrap dual-volume evidence
   remains valid after deleting registration-specific ownership transitions.

Source analysis references from the discussion (not VM validation):
- https://github.com/torvalds/linux/blob/master/fs/xattr.c
- https://github.com/torvalds/linux/blob/master/security/security.c

Aya, bindeps and reproducibly precompiled objects were also discussed. None has
been implemented or chosen by this note. Removing per-step registration and
choosing how the environment builds/loads its policy are separate decisions.

## Source-grounded removal boundary

The subsequent user direction is to simplify conglobate by removing policy
registration. The following is the migration boundary, not a claim that the
replacement has passed validation or that the current executable implements it.

* `src/policy.rs` and `src/policy/{object,installation,pins,bound}.rs` own the
  scoped map ABI, loading, binding and attachment lifetime. An environment-owned
  fixed exception removes these responsibilities from the binary.
* `src/step/preparation.rs` couples `PolicyAssignment`, installation rollback,
  bound-policy cleanup and identity release to storage rollback. Remove the
  policy-only phases; retain mounted storage and child-reaper ownership on
  every setup failure.
* `src/step/pins.rs` combines filesystem identity pins with command user
  namespace ownership. Filesystem pins used solely to prevent policy identity
  reuse can disappear. The namespace handle needed for idmapped mounting and
  command construction must survive independently of policy lifetime. Do not
  remove namespace ownership just because it is presently called a pin.
* `src/process/supervision/registration.rs` is a different registration:
  `join_domain_until` writes the helper into cgroup2 before acknowledging and
  closes the transferred descriptor. Preserve that containment handshake (and
  rename its states around joining a domain). Its packet transport also serves
  admission and volume handoff; deleting it wholesale would break isolation.
* Preserve deadlines, cancellation, descendant draining, storage unmount and
  fallible finalization. There is no longer a policy-disable ordering edge,
  but commands must still be quiescent before storage finalization.

Review the replacement against the original positive and negative VM cases,
including bootstrap's two writable roots. Existing scoped-policy passes do
not establish the behavior of the global exception. Environment validation
must occur before recipe command admission; its exact mechanism remains to be
designed and tested, rather than trusting a pathname or configuration flag.
