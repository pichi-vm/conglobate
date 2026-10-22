Restricted Fedora build followed by exported-root boot — 2026-10-21 UTC
=====================================================================
Build with policy/package.py --seed /tmp/conglobate-rebuilt-package-seed-v2
 --out NEW-BUILD --upgrade-rpms /tmp/conglobate-upgrade-rpms
 --protocol-worker-build /tmp/conglobate-statx-worker-v2 --restricted-bootstrap
 --initramfs-rpms /tmp/conglobate-initramfs-rpms-v1 --root-boot
Then: python3 validation/sandbox/root-boot/boot.py --build NEW-BUILD
 --out NEW-BOOT --kernel-version 7.2.11-200.fc44.x86_64
All referenced RPMs are checked against the existing pinned catalog. No network
or host filesystem share is present in either guest. Build has8GiB RAM and a
450s timeout; boot has2GiB RAM and150s timeout. The build took285.37s guest uptime.

The existing restricted initial DNF installation, synthetic package upgrade,
ACL tests, kernel-install/dracut, command restrictions, descendant cleanup,
fsck/remount and failure-cleanup assertions pass. New recipe-side commands add
an explicit root-only oneshot systemd service, an identity file, and a second
non-hostonly initramfs with ext4/virtio block drivers. Original initramfs-only
probe remains separately generated; it is not included in the root-boot image.

Trusted setup exports the kernel/root-boot initramfs and, only after final
unmount, the2GiB completed filesystem through a separate virtio serial channel.
The diagnostic reader validates compressed length/hash, bounds decompression to
exactly2GiB and writes zero regions sparsely. It never mounts the image on the
host. The second QEMU guest attaches only this owned raw image with a disposable
write overlay, exported kernel/initramfs and serial console. Its root-only
service requires systemd PID1, no /etc/initrd-release, Fedora44 identity, ext4
mounted at /, the installed identity file, device/proc/sys state and installed
bash/systemd/kernel-core packages. Exact success marker occurs once; no probe
failure marker occurs. Systemd shuts down normally in about6.8s guest uptime.
The raw image hash is checked unchanged after boot. Exported kernel SHA256
matches the retained pinned kernel-core RPM payload identity.

This closes the missing representative Fedora root-switch/systemd boot case.
It is not a boot of an OCI carapace through corium, generic recipe integration,
GRUB/UEFI support, target SELinux-policy enforcement, network boot, distribution
version upgrade or all-service qualification. Native raw-root output here and
actual carapace direct-mount tests are separate evidence; their composition is
not asserted. The root-boot fixture deliberately uses SELinux disabled and an
explicit boot-policy service. Existing privileged metadata blockers remain.

The worker is the separately built ACL/finalization/statx prototype, not an
adopted runtime. JSON/ASCII export headers are diagnostic formats, not the final
inventory/attestation interface. Reproduction input hashes, actual generated
scripts, source snapshot, command/logs and output hashes are retained. Large
exports remain at recorded/tmp paths rather than duplicated into the repository.

Three preparations hit storage quota before their guests started (two build
packaging attempts, one boot-copy attempt). Their errors are retained. Generated
roots/boot images from completed older runs were removed only after exact input/
command/result comparisons against retained evidence; cleanup manifests list
those paths. Successful build and boot were not silently retried on a timeout.
