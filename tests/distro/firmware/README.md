# Firmware construction qualification

This recipe builds a new initramfs and runs arma inside an isolated firmware
step, using the completed carapace's CARAPACE environment value. It replaces
both PMI and DTB, then boots the emitted artifact and checks the expected kernel
release from a systemd service. This validates firmware construction from a
supplied kernel, not kernel compilation.

Supply a committed context with the static `arma` executable, `vmlinuz`, its
`kernel.config`, and `initrd-files.tar` containing static corium as `init` plus
the matching dm-zero/snapshot/verity, ext4 and virtio module dependencies under
`usr/lib/modules/<release>`. Acquisition/building of these inputs is external to
conglobate. The reference run used Fedora kernel 7.2.10-200.fc44.x86_64; its exact
binaries and local arma/corium source snapshots are pinned in the preserved
context bundle. Arma had pre-existing local changes; these are included rather
than represented as the clean revision alone.

Run `run.py` with the Fedora cache/pin and this context, then `boot.py --expect
CONGLOBATE_ARMA_FIRMWARE_BOOT_PASS`. The native amd64 build/boot passed. It makes
no Arm64 or confidential-platform boot claim.
