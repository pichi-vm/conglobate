Capability decision probes
==========================
Read REPORT.txt for conclusions, security analysis, limits and sources.
These are disposable-VM experiments, not conglobate production implementation.

Inputs
------
- Existing no-NIC/no-disk/no-share runtime-validation guest seed. The recorded
  default is /var/tmp/conglobate-runtime-pass-v24; --seed can select another.
- For --packages, the prior repository-built Fedora fixture at
  /tmp/conglobate-rebuilt-package-seed-v2/root. See ../package-fixture/README.txt.
  Its 69 RPMs must match ../package-fixture/rpms.json. The runner adds missing
  runtime files without overwriting the seed's worker/patched libfuse.
- For --lsm, the prototype object and loader in
  /var/tmp/conglobate-cap-review-bpf. This fixed local path is a research fixture,
  not a deployment interface. The loader has a fixed outer root UID100000,
  matching the namespace mapping in this fixture.
- The tested kernel has BPF LSM and BTF enabled. SELinux is disabled only inside
  the guest. A failing policy load must never fall back to broader command caps.

Build the optional LSM prototype
-------------------------------
Download/extract the recorded libbpf-devel RPM to an unprivileged sysroot; no
host installation or policy loading is required. Build outputs stay in /var/tmp.
For the recorded paths:

 TMPDIR=/var/tmp/conglobate-cap-review-bpf clang -O2 -g -target bpf \
   -I/var/tmp/conglobate-cap-review-bpf/sysroot/usr/include \
   -c validation/sandbox/capability-review/label-policy.bpf.c \
   -o /var/tmp/conglobate-cap-review-bpf/label-policy.bpf.o
 TMPDIR=/var/tmp/conglobate-cap-review-bpf cc -O2 -Wall -Wextra \
   -I/var/tmp/conglobate-cap-review-bpf/sysroot/usr/include \
   validation/sandbox/capability-review/label-loader.c /usr/lib64/libbpf.so.1 \
   -o /var/tmp/conglobate-cap-review-bpf/label-loader

The BPF policy has a dual BSD-2-Clause/GPL-2.0-only source license and the
GPL-compatible "Dual BSD/GPL" object declaration. The test loader uses libbpf.
Neither changes conglobate's Apache-2.0 license or introduces a production
library dependency. A production Rust integration must preserve forbid(unsafe_code)
at the application boundary; these C/Python helpers are test scaffolding.

Run
---
Each output directory must be new:
 python3 validation/sandbox/capability-review/run.py /var/tmp/NEW-basic
 python3 validation/sandbox/capability-review/run.py /var/tmp/NEW-retained --packages
 python3 validation/sandbox/capability-review/run.py /var/tmp/NEW-lsm --lsm --kill-loader --packages
 python3 validation/sandbox/capability-review/run.py /var/tmp/NEW-failure --lsm --kill-loader --cleanup-failure

The first command tests the no-capability control, a cooperative fixed-FD label
broker, an unfiltered owned tmpfs positive control, and a default-deny restricted
command workload. --packages adds offline DNF installation and installed-program
checks. --lsm removes command SYS_ADMIN from every capability set, uses ordinary
label APIs under the pinned scoped policy, checks a descendant user namespace
with the same underlying root UID is NOT authorized, disables policy after
command exit, and verifies mutation denial. --kill-loader deliberately terminates
only our guest-owned policy loader before commands; pinned guards remain active.

All modes check clean worker shutdown, fsck, an independent read-only kernel ext4
remount and persisted label bytes, then empty recorded process/FUSE resources.
The native remount is a check of these benign generated fixtures, not a proposal
to kernel-mount potentially malformed untrusted output in production.
The runner requires success markers as well as guest completion and returns an
error on a failed probe. It always labels production_qualified=false.

Implementation notes
--------------------
run.py derives a guest matrix from ../runtime-validation/matrix.py. The generated
matrix and actual probe are saved with each evidence run so textual adapter
changes cannot obscure what executed. The guest worker runs in its separate
namespace; the FUSE superblock belongs to the command namespace. FUSE forbids
unrelated user namespaces from accessing this mount even with allow_other, so
the trusted mount helper reports its device number for the LSM target binding.
The supervisor does not bypass that rule or infer a device ID from a wrong view.

probe.py's filter defaults to EPERM. It explicitly permits normal process/file
operations and a limited set of scalar ioctl/prctl operations. Namespace flags
in clone are masked; clone3 returns ENOSYS. The syscall probes use x86_64 numbers;
porting them and qualifying the policy on Arm remain work. Absence of an interface
from this experimental allowlist can break valid recipes; it is not a final
compatibility contract. The label broker is cooperative and fixed-inode only,
not a claimed transparent seccomp-notification implementation.

Recorded evidence
-----------------
../reproduced/capability-review/ contains final retained-capability and capless-LSM
runs, inputs/command/verdict files, generated test sources, object/tool hashes,
selected kernel source snapshots, the source-archive digest, capability-site
inventory and earlier diagnostic logs. Historical failures include incomplete
prctl/ioctl compatibility, a fixture assertion expecting an RPM absent from its
input set, and policy-loader attempts to stat a FUSE mount from an unrelated
user namespace. They are not silently discarded or counted as successful runs.

Review regression tests
-----------------------
The LSM probe checks that longer attribute names beginning with security.selinux
are denied for both set and remove. The exact attribute must continue to work.
--cleanup-failure is an LSM-only fault injection: a command forks a sleeping
child; the supervisor observes live command processes and injects an error.
The failure path kills the command namespace processes, waits for quiescence and
reaps its launcher BEFORE changing policy, unmounting, or releasing namespace
identity. Failure to establish scope or meet the deadline leaves the policy
pinned and aborts the disposable VM test. It does not attempt permissive cleanup.
This does not qualify abrupt supervisor death or partial policy installation.
