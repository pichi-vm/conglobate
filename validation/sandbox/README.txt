Sandbox design experiments — not the production executor

Current design: ../../SANDBOX-DESIGN.txt and ../../DESIGN.md.
Latest sibling-sandbox topology: two-sandboxes/README.txt. Each older fixture
retains its original configuration and limits; it does not establish a pass
for the final composed design.

These are disposable-VM probes. Never run init or worker scripts on a host.
The VM has no host disk, shared directory, or network interface. Its image,
loop devices and device-mapper snapshot are backed by guest tmpfs. The host
SELinux policy remains enforcing; only the disposable guest disables SELinux.

The trusted components are the guest kernel, outer harness, and fuse2fs.
The command script is adversarial. Python ctypes is test instrumentation;
it is not a proposal to permit unsafe code in conglobate.

Current reproduction uses guest.py and the per-fixture README files below.
The original /tmp run scripts are historical evidence, not prerequisites.
For the larger offline Fedora build, see package-fixture/README.txt and
policy/README.txt; package-upgrade/README.txt covers the version-change probe.

The run scripts package their root trees as newc/gzip initramfs and boot stock
Fedora kernel 7.2.10-200.fc44.x86_64 under QEMU KVM with 4GiB RAM and two CPUs.
Arguments: -nodefaults -no-reboot -display none -serial stdio -monitor none
-kernel vmlinuz -initrd initramfs.gz
-append 'console=ttyS0 rdinit=/init selinux=0 panic=-1 quiet'
An outer timeout bounds the VM. QEMU exit zero is NOT a successful test.
Read each probe result and guest exit. Diagnostic probes may continue after
an expected or investigated failure; the final VALIDATION_PASS marker alone
must not be treated as validation of every probe.

Dependencies in the root tree are copied with their ldd dependencies; the
Fedora kernel's fuse, loop and dm_snapshot modules are included with module
indexes. The package test installs the cached RPM collection inventoried in
/tmp/conglobate-idmap-evidence/packages.json. Those large binaries and RPMs
are not checked into this repository. The repository now contains package-fixture/build.py and a pinned RPM catalog
for assembling this workload without the original temporary seed. External
RPMs and installed kernel/tool versions are still explicit prerequisites;
this is not a hermetic or independently provisioned CI environment.

The cancellation experiment stops fuse2fs with SIGSTOP, observes a pending
request in fusectl, aborts from the initial user namespace, observes a failed
client operation, kills/reaps the worker and verifies connection removal.
No guest output is accepted after abort.

Additional retained fixtures:
- network/: three isolated guest network namespaces, fake upstream DNS/HTTP,
  NAT/filter rules, tmpfs/FD/cgroup exhaustion, IPC/UTS/shared-memory probes.
  Reproduction: guest.py network (see below).
- concurrent/: two separately mapped filesystem workers; retained abort FD
  for A; abort A while B remains writable; clean unmount/fsck B and no leaked
  FUSE connection. Reproduction: guest.py concurrent (see below).
- acl-worker + acl-comparison-results.txt: same ACL grant/inheritance probes
  in initial and mapped user namespaces. Both show the backend limitation.

Repository-based reproduction (preferred for small fixtures)
-----------------------------------------------------------
From the repository root, with QEMU/KVM access, an installed kernel image and
matching modules, and the tools named by guest.py:

  python3 validation/sandbox/guest.py cancellation --kernel-version 7.2.10-200.fc44.x86_64 --out /tmp/new-cancellation-fixture --run

List current cases with guest.py --help. Each --out must be new.
The builder packages local trusted binaries/libraries and selected modules,
records SHA256 for each packaged file plus symlinks in inputs.json, and writes
command.json and results.txt. It invokes no privileged host operations. QEMU
has no disk, NIC or host directory exports. Guest storage is tmpfs.

The acl and faults cases reproduce observed defects; runner success means the
expected diagnostic outcomes were recorded, NOT that the backend is acceptable.
The ACL case checks failures in both initial and mapped user namespaces. The
network case also includes bounded resource-exhaustion and IPC/UTS probes.
The runner uses a 150-second outer timeout and verifies guest/probe markers,
not QEMU return status alone. Source/package version comparisons still matter:
a changed dependency may resolve a defect and invalidate an expected failure.

The larger offline Fedora fixture uses package-fixture/build.py followed by
policy/package.py, as documented in their READMEs. It has a separate packager.

Evidence integrity follow-up
----------------------------
guest.py now requires exactly one complete GUEST_EXIT 0 line. A nonzero,
malformed, prefixed or duplicate exit record is rejected, even if all expected
probe marker substrings appear. Case-specific assertions still determine what
a successful diagnostic run means; there is no generic security-pass marker.
Run python3 validation/sandbox/check-harness.py for the acceptance regressions.

Before packaging, guest.py captures its own source, selected fixture scripts,
production Rust sources and manifest/lock files, and the three helper projects'
Rust sources/manifests/locks. It compares them again after builds/copies and
rejects changed input. The catalog lists sources and SHA256; sources/ retains
the bytes. It conservatively includes helper projects unused by some cases.
The packaged executable/library/module hashes remain in files as before.

Validated run and retained snapshots: reproduced/evidence-harness. This new
source catalog applies to newly generated runs. Earlier runs retain only their
original inputs/helper hashes; do not infer that they used current source.
External dependency checkout integrity, compiler identity, worker source-build
provenance and all environment inputs are not established by this addition.
The source comparison detects ordinary concurrent edits, not an adversarial
change-and-restore race. This is a trusted test harness, not an attestation
format or a substitute for the production inventory contract.
