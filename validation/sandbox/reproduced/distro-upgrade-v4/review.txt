Fedora43 ->44 distribution transaction under restricted execution
=================================================================
Result: a real repository-solved version upgrade passes under the mapped FUSE
worker and restricted command policy. This is distinct from the synthetic two-
version RPM scriptlet test. Guest has no NIC or host share; all repositories are
read-only local inputs. Guest exit0 at98.43s.

Exact input reproduction:
 python3 validation/sandbox/distro-upgrade/download.py \
   --inputs NEW-INPUT-DIRECTORY \
   --lock validation/sandbox/reproduced/distro-upgrade-v4/download-inputs/rpms.json
 python3 validation/sandbox/policy/package.py \
   --seed /tmp/conglobate-rebuilt-package-seed-v2 --out NEW-GUEST-DIRECTORY \
   --restricted-bootstrap --pinned-runtime \
   --protocol-worker-build /tmp/conglobate-statx-worker-v2 \
   --distro-upgrade-inputs NEW-INPUT-DIRECTORY
The seed can be recreated using package-fixture/build.py. Downloader checks exact
locked RPM bytes/lengths and generates local repository metadata; package.py
checks the resulting repository-file hashes before packaging, then inventories
all guest files. Regenerated repository metadata need not be bit-identical.
The locked-download path was checked separately against two small real RPMs;
the233-package successful input set was originally fetched during resolution.

Resolution used explicit Fedora releases43/44 Everything x86_64 repositories,
no host repository configs, no plugins, no weak dependencies and excluded i686.
Initial download resolves the named package set (bash/coreutils/python3/attr/acl/
libcap/rpm/fedora-release/systemd/dnf5). Target closure must ALSO cover the source
package names, including optional packages already installed in43. The successful
lock contains233 RPMs (114 source candidates,119 target candidates), about122MiB.
Candidate counts are not installed-package counts. Alternative implementations
must remain candidates; installing every downloaded filename creates conflicts.

Guest assertions:
- Initial installation via the source43 local repo into the FUSE output mount;
  RPM fedora-release version43 and installed /etc/os-release VERSION_ID43.
- A real distro-sync against only the target44 local repo, without skip-broken,
  nodeps, or an allowerasing workaround.
- RPM fedora-release version44, /etc/os-release VERSION_ID44, fc44 glibc and
  successful DNF dependency check. Before/after package lists are written in the
  image; the complete transaction log is retained.
- Author-owned canary content and mode0640/UID1234/GID2345 survive the upgrade
  and a later direct kernel ext4 remount after restricted processes/worker exit.
- Existing paired ACL, foreign SELinux label, file capability, isolation,
  descendant teardown, fsck, unchanged origin and worker-death cleanup checks pass.
Tool commands execute from the separate trusted tool root. Both installation and
upgrade run after namespace entry/seccomp/no_new_privs; no network is provided.

Retained unsuccessful controls:
 v1: missing repository mountpoint directories; no installation began.
 v2: installing all downloaded candidates explicitly conflicted (coreutils vs
     coreutils-single and alternate sysusers implementations). Corrected by
     asking DNF to solve named packages from the source repository.
 v3: source installation passed; target closure omitted systemd-pam installed
     by43, so distro-sync correctly refused. Expanded target inputs; no package
     erasure/skip workaround. Initial and completed locks remain retained.
 v4: full workload and kernel-remount assertions pass.
An initial DNF URL-resolution invocation rejected a comma-separated architecture
option; using explicit name.arch package specs and i686 exclusion resolved it.

Limits: representative minimal Fedora userspace, not every distribution/package,
booted system-upgrade plugin, desktop migration, kernel upgrade, service startup
or SELinux enforcement. Kernel/initramfs and corium boot have separate evidence;
this run does not claim to combine them with the distribution upgrade. RPM
signature enforcement is disabled in the disposable fixture, as in the existing
package workload; HTTPS download plus byte locking is not a new production
signature/attestation policy. Existing mapped-FUSE special metadata and final
executor integration blockers remain. No backend or production policy adoption.

Primary references consulted:
https://dnf5.readthedocs.io/en/latest/commands/download.8.html
https://fedoraproject.org/wiki/Upgrading_Fedora_using_package_manager
https://docs.fedoraproject.org/en-US/quick-docs/upgrading-fedora-offline/
These describe dependency downloading and distribution synchronization. This
fixture tests image construction semantics; it does not prescribe live-host
upgrade procedures or claim Fedora QA certification.
