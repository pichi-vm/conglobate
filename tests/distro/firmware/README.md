# Firmware construction qualification

This plan constructs an initramfs and runs Arma inside an isolated stage. It
extracts the resulting embedded-DTB PMI, selects the separate userspace root,
and overlays native bootconfig. Final composition binds `corium.root`; no shell
root-hash variable is needed. Boot verification checks the expected kernel
release from a systemd service. This tests firmware construction from a supplied
kernel, not kernel compilation.

Supply a committed context with the static `arma` executable, `vmlinuz`, its
`kernel.config`, and `initrd-files.tar` containing static corium as `init` plus
the matching dm-zero/snapshot/verity, ext4 and virtio module dependencies under
`usr/lib/modules/<release>`. Acquisition/building of these inputs is external to
conglobate. Use a Corium build with native bootconfig and GPT support and an
Arma build supporting `--dtb attached --cmdline bootconfig`.

Run `run.py` with the Fedora cache/pin and this context, then `boot.py --expect
CONGLOBATE_ARMA_FIRMWARE_BOOT_PASS`. A build alone is not a boot pass. This
fixture targets native amd64; Arm64 and confidential platforms require their
own qualification.
