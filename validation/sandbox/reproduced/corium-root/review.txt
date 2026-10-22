Restricted Fedora root imported by conglobate, booted through corium
=================================================================
Reproduce:
 cargo build --locked --manifest-path validation/sandbox/publication/helper/Cargo.toml --bin import-root
 cargo build --locked --offline --manifest-path ../corium/Cargo.toml --release --target x86_64-unknown-linux-musl
 python3 validation/sandbox/corium-root/boot.py \
   --raw-boot /tmp/conglobate-fedora-root-boot-v2 \
   --rpms /tmp/conglobate-initramfs-rpms-v1 \
   --corium ../corium/target/x86_64-unknown-linux-musl/release/corium \
   --importer validation/sandbox/publication/helper/target/debug/import-root \
   --out NEW-DIRECTORY

Inputs are the already exported restricted-build Fedora44 raw filesystem/kernel,
matching kernel-core/modules-core RPMs, and a freshly rebuilt static corium from
b1d2d24a8db9cf845c79a3cabc7cdd5dd6f0a275. Its carapace dependency matches the
conglobate fixture pin99caf7941ac8930a5761f6f843b6d8e53dd89587. The source tree was
clean before/after the corium build. This does not modify that repository.

No host mount, loop/device-mapper setup, NIC or host directory share is used.
The diagnostic import-root helper calls the actual conglobate Scute::import API
on the owned2GiB raw filesystem. That uses carapace planning and conglobate's
streaming COW/verity path. GPT data/hash partition UUIDs encode the digest halves
using the existing corium test convention; this is diagnostic disk packaging,
not a new OCI format or full conglobate recipe execution.

The fresh minimal initramfs contains static corium and the dependency closure
from the matching kernel RPMs (only dm-verity and reed_solomon are modules in
this kernel; other requested drivers are built in). RPM hashes were compared
with the retained restricted-build input catalog. QEMU attaches the generated
GPT read-only and passes corium=<root hash>. Source and disk digests are checked
again after the guest exits. The guest has2GiB RAM and a150-second deadline.

Run v3 passes: corium discovers/assembles/mounts/pivots, then executes systemd.
The existing root-only service confirms Fedora identity, installed packages,
ext4 root, no initrd marker, and proc/sys/device availability, then emits exactly
one FEDORA_ROOT_SWITCH_SYSTEMD_PASS7.2.11-200.fc44.x86_64 and powers off. The actual
marker contains a space before the release. Completion occurs at about4.63s.
Input raw filesystem and packed read-only disk digests remain unchanged.

This closes single-scute bootstrap-output/corium/root-switch composition. It does
NOT close derived multi-scute boot, OCI transport, distro-version upgrade,
SELinux enforcement, generic executor or final firmware/DTB integration.
The log explicitly shows systemd-homed/logind failures, and remaining busy DM
devices during shutdown before poweroff. These are not hidden: the probe proves
root handoff and its stated checks, not all-service health or DM teardown in a
live continuing kernel. Do not infer that read-only root alone caused service
failures without their journal diagnostics. Corium's read-only-root contract
supplies no writable overlay; that remains image policy.

Two preparations stopped before any guest: v1 selected the wrong RPM module
prefix; v2's importer returned1 while storage quota was full. Its stderr was
not captured by the original exception path, so the precise importer errno is
unknown. The runner now records importer output before checking status. Partial
output hashes and quota/cleanup notes are retained. Cleanup removed only owned
generated files after matching prior retained evidence; raw input stays intact.

Retained evidence: reproduced/corium-root, with source catalog, commands, module/
RPM/input identities, partition recipe, full boot log, result and output digests.
Preparation failures live in reproduced/corium-root-preparation. Large outputs
remain at the recorded/tmp paths. Strict importer clippy and Python compilation
pass. No production behavior or backend architecture was changed for this probe.
