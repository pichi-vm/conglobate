# Concrete network cleanup histories

Baseline: 08f1189. Continued errors-api-002/003 remediation.

Network setup, NAT rollback and command-network cleanup no longer format their
child failures into new string errors. Crate-private read-only diagnostics
accessors expose the existing immutable histories. Initial setup retains a full
child history; subsequent transitions append only the newest error, avoiding
replaying older history into its parent. Process wait failures remain concrete
io::Error payloads. Firewall and resolver cleanup now retain their original
error plus later failed attempts instead of replacing the original on retry.
No new owner, authority, deadline, cleanup retry or admission path was added.

Source review: setup still owns and reaps creators before command teardown;
network cleanup still closes policy, connection and resolver in that order;
NAT cleanup retains its connection; all failures retain their original deadline
and exact pending owner. The changes are typed diagnostics and immutable history
extension, not state-machine transitions. New accessors remain crate-private.

Validation:

- Initial library run passed 145 tests with an obsolete io-import warning;
  preserved log, then removed that import.
- 146 library tests passed, including a new ordinary-filesystem resolver test
  forcing three removal failures, checking errno/history and restoring the
  fixture before successful cleanup using the retained owner.
- Strict all-target Clippy passed.
- The first network-composite VM run failed its existing NAT-installation-context
  assertion. Copying the underlying history had removed the operation label.
  Preserved the failed run; did not weaken the assertion.
- Added an internal SetupCause error enum with typed source histories and explicit
  veth/connection/configuration/NAT labels. Repeated 146 library tests and strict
  Clippy passed after this fix. Corrected VM result is recorded separately.
- The maintained VM fixture additionally checks timeout kind and exact history
  length during repeated expired network cleanup, while retaining its existing
  interface/firewall ownership assertions.

Commands use CARGO_BUILD_JOBS=2, TMPDIR=/var/tmp/conglobate-rewrite-tmp,
CARGO_TARGET_DIR=target/m7-stable, serial cargo +stable test --lib and clippy
--all-targets -- -D warnings. VM runs use tests/lifecycle/run.py --case
network-composite under ordinary native amd64 KVM; no host namespace mutation.

This does not exhaust all netlink, firewall or helper-process failure injections.
The final public diagnostic taxonomy/ownership audit remains outstanding.
No full adherence pass is claimed; M4 remains 0/3.

Corrected VM run v2 passed all maintained assertions, including the unchanged
NAT-installation-context assertion and new typed timeout/history checks. All 156
compared source files match v2. Failed v1 differs only in network/setup.rs (the
context fix), recorded explicitly. Both runs and local large-asset hashes are
preserved. These checks do not close the final diagnostic taxonomy audit.

Terminology clarification prompted by the user: ResolverInput/ResolverSource
manage the supplied resolv.conf bytes and their read-only sandbox mount. They do
not implement a DNS resolver, DNS proxy, cache, daemon or server selection.
The VM environment can provide systemd-networkd/systemd-resolved. Conglobate's
current private per-step namespace/link/NAT lifecycle remains distinct from VM
uplink management. No change of networking ownership is implied by this patch.
