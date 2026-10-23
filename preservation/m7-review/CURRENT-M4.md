# Current M4 gate work queue

At 0345372 plus current two-root validation. M4 remains 0/3. This replaces the
execution queue of m4-adherence-168b2a2/REVIEW.md, not its historical observations.
No full new adherence pass has been awarded.

1. **UID/GID contract:** current ImageOwnership validates all inherited inode and
   access/default ACL IDs before receiver admission; maintained unmapped-owner,
   unmapped-acl and unmapped-default-group tests exist. All three coordinators
   still use 100000/65536. Asked whether environment-configurable mapping or a
   fixed initial supported range is desired; answer pending, no choice inferred.
2. **Network boundary:** binary no longer allocates routes/NAT, so historical
   allocation-collision fixes are superseded. Environment owns reachable
   destinations and ingress policy. Namespace pinning/type rejection, DNS,
   enabled reuse, offline separation, capability denial, timeout/nonzero cleanup
   are covered in external-network and external-network-followup. IPv6/ingress
   remain environment qualification, not proved by a namespace descriptor.
3. **Resolver mutation:** resolver_target unit tests cover dangling targets,
   replaced scaffold entries and pinned parent replacement. Command-time dangling-target and pinned-parent replacement VM coverage now passes
   (resolver-lifecycle/). Replacement of an owned scaffold rejects publication but
   exposed resolver-cleanup-diagnostic-001. It is now resolved by the bounded
   trusted cleanup-failure message; current positive/negative VM verifiers pass
   (cleanup-failure-protocol/). No full arbitrary-race/platform claim is added.
4. **Label admission:** current two-root denial now passes (m4-second-root-current).
   Current missing-policy negative passes all six checks with no output
   (m4-missing-policy-current); its source comparison is recorded alongside raw evidence. Do not
   confuse admission-probe success with enforcing target-distro boot validation.
5. **Security coverage:** seeded-key-confinement proves populated supervisor key
   control before/after a command's empty key view. time-namespace-control adds
   privileged unshare control. sched_debug-present/arm64 coverage remains absent.
   splice/tee/vmsplice compatibility and trusted-formatter scope remain explicit
   findings, not demonstrated breakouts or silently changed policy.
6. **Review completeness:** historical-actions-current.json now indexes all 111
   original IDs exactly once. 65 added entries retain historical dispositions but
   explicitly lack current-source revalidation. Existing updated entries retain
   their source/evidence status. Reconcile changed scopes; do not infer closure
   from historical reject/fixed labels. Then freeze and run three full reviews.
7. **Preservation and dependent milestones:** all local new evidence remains
   recoverable in current paths/history, but local asset hashes are not durable
   remote archives. Retire nothing until M8 accounting succeeds. M5/M6 gates,
   distro builds/boots, portable CI and platform qualification remain outstanding.

The previous goal turn made implementation and disposition progress. This turn
adds current negative execution evidence, a complete finding index and a concrete
remaining queue. It is neither a status-only turn nor a claim of goal completion.
