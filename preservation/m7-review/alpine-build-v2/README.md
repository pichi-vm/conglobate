# Alpine bootstrap, APK and OpenRC boot

Current Rust source at 311f587 (unchanged from green CI at 9a70e50) built a new
Alpine root using the Fedora tool carapace and trusted ext4 formatter. Bootstrap
installed tar in its disposable tools, verified/extracted the pinned Alpine
3.24.2 minirootfs, then the carapace stage ran APK under musl and installed
alpine-base 3.24.3/OpenRC. Both firmware files were packaged from the existing
Fedora kernel/corium pair; tool ancestry was not inherited as root layers.

Build/export and separate native amd64 boot passed. OpenRC's local service
checked Alpine identity, installed package records and persisted root ownership
before printing CONGLOBATE_ALPINE_OPENRC_BOOT_PASS. The harness terminated the
VM after that marker; guest shutdown is not qualified. Existing firmware emits
module/alignment warnings preserved in the console; the required boot completes.

This covers Alpine userspace, bootstrap, APK package scripts, musl command
execution and OpenRC boot. It does not qualify Alpine kernel generation, Arm,
confidential hardware or all Alpine workloads. The first attempt's missing-tar
failure is preserved separately. No conglobate implementation change was needed.
Large artifact/runtime bytes remain local with hashes pending M8 durability.
