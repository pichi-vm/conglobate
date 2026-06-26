#!/bin/sh
# SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
# SPDX-License-Identifier: Apache-2.0
#
# Assemble the official pichi build image initramfs + kernel. Runs INSIDE an
# arch-matched Alpine container (see build-image.sh): the host stages the
# prebuilt static-musl conglobate in /work/conglobate and collects the kernel,
# config and uncompressed cpio from /work/out afterwards. The host then seals
# them into the PMI with arma (arma needs only the files, not a musl runtime,
# so it stays on the host).
#
# The build image is a PMI-only pichi VM whose /init is conglobate. conglobate
# mounts only ext4 (the working snapshot) and virtiofs (/context, /output) and
# writes erofs purely in userspace via mkfs.erofs, so the kernel needs fuse,
# virtiofs, ext4, loop and the dm-snapshot stack — but NOT erofs or dm-verity.
# apk linux-virt ships them all as modules; VIRTIO transport is built in.
set -eu

: "${ALPINE_PKGS:=linux-virt erofs-utils kmod cpio}"
apk add --no-cache $ALPINE_PKGS >/dev/null
KV=$(ls /lib/modules)
echo "assemble: kernel $KV ($(uname -m))"

IRFS=/tmp/irfs
rm -rf "$IRFS"
mkdir -p "$IRFS/modules" "$IRFS/usr/bin"

# /init = conglobate (PID 1). Kept static-musl, so no loader needed for it.
install -m 0755 /work/conglobate "$IRFS/init"

# Kernel modules in dependency order. `modprobe --show-depends` lists each
# module's prerequisites before it; concatenating the top-level modules and
# de-duplicating (first occurrence wins) yields a valid global load order.
# crc/crypto helpers are arch-specific, so resolve them here in the
# target-arch container rather than hardcoding a list.
modlist=/tmp/modlist
: >"$modlist"
for top in virtiofs ext4 loop dm-snapshot; do
	modprobe -S "$KV" --show-depends "$top" 2>/dev/null \
		| sed -n 's|^insmod \([^ ]*\).*|\1|p' >>"$modlist"
done

i=0
awk '!seen[$0]++' "$modlist" | while read -r ko; do
	base=$(basename "$ko")
	base=${base%.gz}
	base=${base%.ko}
	nn=$(printf '%02d' "$i")
	zcat "$ko" >"$IRFS/modules/${nn}-${base}.ko"
	echo "  module ${nn}-${base}.ko"
	i=$((i + 1))
done

# mkfs.erofs (userspace erofs writer) + its shared libraries. conglobate is
# static, so these libs serve only mkfs.erofs.
install -m 0755 /usr/bin/mkfs.erofs "$IRFS/usr/bin/mkfs.erofs"
ldd /usr/bin/mkfs.erofs | awk '
	/=>/ && $3 ~ /^\// { print $3 }       # libfoo.so => /path
	!/=>/ && $1 ~ /^\// { print $1 }      # /lib/ld-musl-*.so (loader)
' | sort -u | while read -r lib; do
	install -D -m 0755 "$lib" "$IRFS$lib"
	echo "  lib $lib"
done

# initramfs (newc cpio, UNcompressed — arma rejects compressed initrds; the
# PMI is no-compression by design) + kernel + its config (arma reads the
# config to size virtio-mmio transport slots).
mkdir -p /work/out
(cd "$IRFS" && find . | cpio -o -H newc 2>/dev/null) >/work/out/initramfs.cpio
cp "/boot/vmlinuz-virt" /work/out/vmlinuz
cp "/boot/config-$KV" /work/out/config
echo "assemble: initramfs $(wc -c </work/out/initramfs.cpio) bytes"
echo "assemble: kernel + config + cpio staged in /work/out"
