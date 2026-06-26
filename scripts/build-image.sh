#!/usr/bin/env bash
# SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
# SPDX-License-Identifier: Apache-2.0
#
# Build one architecture's official pichi build image (a PMI-only pichi VM
# whose /init is conglobate) and write it to $OUT/boot.pmi.
#
# Runs natively on a runner of the target arch — cross-compilation and qemu are
# deliberately avoided, so the CI matrix uses one native runner per arch
# (ubuntu-24.04 + ubuntu-24.04-arm). Steps:
#   1. build conglobate as a static-musl binary (it becomes /init in the
#      initramfs, so it must run in the PMI's minimal musl userland);
#   2. assemble the initramfs + collect the kernel inside an arch-matched
#      Alpine container (scripts/assemble.sh);
#   3. build arma on the host and seal the kernel + initramfs into the PMI
#      (arma only consumes the files, so it needs no musl runtime).
#
# Inputs: ARMA_DIR (path to an arma checkout). Optional: OUT (default
# ./image-out), PMI_CMDLINE.
set -euo pipefail

ARMA_DIR="${ARMA_DIR:?set ARMA_DIR to an arma checkout}"
OUT="${OUT:-$(pwd)/image-out}"
PMI_CMDLINE="${PMI_CMDLINE:-console=ttyS0 panic=-1}"
root=$(cd "$(dirname "$0")/.." && pwd)

case "$(uname -m)" in
	x86_64) rust_target=x86_64-unknown-linux-musl;  platform=linux/amd64 ;;
	aarch64) rust_target=aarch64-unknown-linux-musl; platform=linux/arm64 ;;
	*) echo "build-image: unsupported arch $(uname -m)" >&2; exit 1 ;;
esac
echo ">>> building pichi build image for $platform ($rust_target)"

# 1. conglobate /init — static-musl. The rustup musl targets are self-contained
#    (bundled musl + rust-lld), so no cross C toolchain is needed when the
#    runner is native to the target arch.
rustup target add "$rust_target" >/dev/null
(cd "$root" && cargo build --release --target "$rust_target" --bin conglobate)

# 2. initramfs + kernel, inside a native (no-qemu) Alpine container.
work=$(mktemp -d)
mkdir -p "$work/out"
install -m 0755 "$root/target/$rust_target/release/conglobate" "$work/conglobate"
cp "$root/scripts/assemble.sh" "$work/assemble.sh"
podman run --rm --platform "$platform" -v "$work:/work:z" alpine:3.21 sh /work/assemble.sh

# 3. arma seals the PMI (host build; honours its own pinned nightly toolchain
#    via $ARMA_DIR/rust-toolchain.toml).
(cd "$ARMA_DIR" && cargo build --release --bin arma)
arma="$ARMA_DIR/target/release/arma"

mkdir -p "$OUT"
"$arma" build \
	--kernel "$work/out/vmlinuz" \
	--initrd "$work/out/initramfs.cpio" \
	--config "$work/out/config" \
	--cmdline "$PMI_CMDLINE" \
	"$OUT/boot.pmi"
echo ">>> $OUT/boot.pmi ($(wc -c <"$OUT/boot.pmi") bytes)"
