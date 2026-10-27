#!/usr/bin/env bash
# SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
# SPDX-License-Identifier: Apache-2.0

# Build the Conglobate appliance image — a pichi artifact (carapace + detached
# DTB + PMI + launch config). The initramfs is static Corium as /init; it
# pivots into this rootfs, whose PID 1 is systemd. systemd then loads virtiofs
# and fuse from the rootfs module tree and mounts tags repo/cache/output.
#
# ONE mkosi run builds the Fedora 43 rootfs from packages (not from the
# published fedora carapace). build.sh installs /usr/bin/conglobate and the
# units via mkosi.extra, then imports the image. Registry push is not done
# here; CI pushes ghcr.io/pichi-vm/conglobate on main only.
#
# Requires: mkosi, pichi, arma (ARMA=<path> or on PATH), cargo + rustup,
# cpio, dumpe2fs (e2fsprogs). Runs rootless (userns) aside from mkosi's own
# user namespace.
set -euo pipefail

RELEASE=43
IMAGE="${IMAGE:-ghcr.io/pichi-vm/conglobate}"
ARMA="${ARMA:-arma}"
CORIUM_GIT="${CORIUM_GIT:-https://github.com/pichi-vm/corium}"
# Corium 0.2: Linux 7+, corium=<hash>, GPT discovery and /sbin/init handoff.
# Same pin as the fedora base image.
CORIUM_REV="${CORIUM_REV:-b1d2d24a8db9cf845c79a3cabc7cdd5dd6f0a275}"
CORIUM_TOOLCHAIN="${CORIUM_TOOLCHAIN:-stable}"
CONGLOBATE_TOOLCHAIN="${CONGLOBATE_TOOLCHAIN:-1.94.0}"

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/.." && pwd)"
cd "$here"

crate_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$repo/Cargo.toml" | head -1)"
if [ -z "$crate_version" ]; then
	echo "!!! could not read package version from $repo/Cargo.toml" >&2
	exit 1
fi
if [ -n "${CONGLOBATE_VERSION:-}" ] && [ "$CONGLOBATE_VERSION" != "$crate_version" ]; then
	echo "!!! CONGLOBATE_VERSION=$CONGLOBATE_VERSION does not match Cargo.toml $crate_version" >&2
	exit 1
fi
VERSION="$crate_version"

case "$(uname -m)" in
	x86_64)  MUSL=x86_64-unknown-linux-musl ;;
	aarch64) MUSL=aarch64-unknown-linux-musl ;;
	*) echo "!!! unsupported arch $(uname -m)" >&2; exit 1 ;;
esac

work="$here/.build"
extra="$here/mkosi.extra"
cleanup_extra() {
	if [ -e "$extra" ]; then
		chmod -R u+w "$extra" 2>/dev/null || true
		rm -rf "$extra"
	fi
}
trap cleanup_extra EXIT
cleanup_extra
# mkosi's rootfs tree is owned by us but read-only (dir mode 0555); make it
# writable before removing a previous run's output.
[ -e "$work" ] && chmod -R u+w "$work"
rm -rf "$work"
mkdir -p "$work"

# ---- 1. conglobate release binary + static Corium init -------------------
if [ -n "${CONGLOBATE_BIN:-}" ]; then
	conglobate_bin="$CONGLOBATE_BIN"
else
	cargo "+$CONGLOBATE_TOOLCHAIN" build --locked --release --bin conglobate --manifest-path "$repo/Cargo.toml"
	conglobate_bin="$repo/target/release/conglobate"
fi
if [ ! -x "$conglobate_bin" ]; then
	echo "!!! conglobate binary is not executable: $conglobate_bin" >&2
	exit 1
fi
if ldd "$conglobate_bin" | grep -q 'not found'; then
	echo "!!! conglobate is missing shared libraries" >&2
	ldd "$conglobate_bin" >&2
	exit 1
fi

if [ -n "${CORIUM_BIN:-}" ]; then
	corium_bin="$CORIUM_BIN"
else
	rustup target add --toolchain "$CORIUM_TOOLCHAIN" "$MUSL"
	cargo "+$CORIUM_TOOLCHAIN" install --locked --git "$CORIUM_GIT" --rev "$CORIUM_REV" \
		--target "$MUSL" --root "$work/corium" --quiet corium
	corium_bin="$work/corium/bin/corium"
fi

# ---- 2. stage units, virtiofs autoload, and the engine into the rootfs ---
inst="$extra/usr/lib/systemd/system"
mkdir -p "$extra/usr/bin" \
	"$inst/local-fs.target.wants" \
	"$inst/multi-user.target.wants" \
	"$extra/usr/lib/modules-load.d"
install -m0755 "$conglobate_bin" "$extra/usr/bin/conglobate"
install -m0644 "$here/units/"*.mount "$here/units/"*.service "$inst/"
for tag in repo cache output; do
	ln -s "../run-pichi-${tag}.mount" "$inst/local-fs.target.wants/run-pichi-${tag}.mount"
done
ln -s ../conglobate-build.service "$inst/multi-user.target.wants/conglobate-build.service"
printf '%s\n' fuse virtiofs > "$extra/usr/lib/modules-load.d/pichi-virtiofs.conf"

# mkosi copies mkosi.extra/ into the image before mkosi.finalize.
mkosi --force --release "$RELEASE" --output-dir "$work"

find_one() {
	local desc="$1" f
	shift
	for f in "$@"; do
		[ -e "$f" ] && { printf '%s' "$f"; return; }
	done
	echo "!!! could not find $desc in $work; mkosi produced:" >&2
	ls -1 "$work" >&2
	exit 1
}
root="$(find_one 'root ext4 partition' "$work"/*.root-*.raw "$work"/*.root.raw)"
kver="$(cat "$work/kver")"
echo ">>> kernel $kver  root $(basename "$root")"

bs="$(dumpe2fs -h "$root" 2>/dev/null | sed -n 's/^Block size: *//p')"
[ "$bs" = "4096" ] || { echo "!!! root ext4 block size $bs != 4096" >&2; exit 1; }

# ---- 3. custom initramfs (static Corium + pivot modules, not virtiofs) ----
stage="$work/initrd-root"
rm -rf "$stage"
mkdir -p "$stage"
install -m0755 "$corium_bin" "$stage/init"
[ -d "$work/initrd-mods" ] && cp -a "$work/initrd-mods/." "$stage/"
( cd "$stage" && find . -print0 | cpio --null --create --format=newc --quiet ) > "$work/initrd.cpio"
initrd="$work/initrd.cpio"
echo ">>> initrd $(du -h "$initrd" | cut -f1)  modules: $(find "$stage" -name '*.ko*' | wc -l)"

# ---- 4. import the carapace once; read its top root hash ------------------
carapace="conglobate:$VERSION-carapace"
pichi import raw "$root" -t "$carapace" --quiet
hash="$(pichi inspect "$carapace" \
	--format '{{ manifest.annotations["dev.pichi.carapace.verity.hash"] }}')"
[[ "$hash" =~ ^[[:xdigit:]]{64}$ ]] || { echo "!!! invalid carapace verity root: $hash" >&2; exit 1; }
echo ">>> carapace $carapace  root $hash"

# ---- 5. detached-mode PMI + base DTB -------------------------------------
# corium= is the trust anchor Corium reads. systemd.* applies to PID 1 after
# pivot: the carapace root is read-only, so /var must be volatile. virtiofs
# mounts are units in the rootfs, not kernel command-line options.
cmdline="corium=$hash console=hvc0 systemd.volatile=state systemd.unit=multi-user.target systemd.show_status=false"
printf '%s\n' "$cmdline" > "$work/cmdline"
"$ARMA" build \
	--kernel "$work/vmlinuz" \
	--config "$work/kernel.config" \
	--initrd "$initrd" \
	--cmdline "$cmdline" \
	--dtb "$work/base.dtb" \
	"$work/boot.pmi"

# ---- 6. combine PMI + DTB + config onto the carapace ---------------------
rev="${REVISION:-$(git -C "$repo" rev-parse HEAD 2>/dev/null || echo unknown)}"
pichi import pmi "$work/boot.pmi" \
	--dtb "$work/base.dtb" \
	--config "$here/config.json" \
	--carapace "$carapace" \
	-a "org.opencontainers.image.source=https://github.com/pichi-vm/conglobate" \
	-a "org.opencontainers.image.revision=$rev" \
	-a "org.opencontainers.image.version=$VERSION" \
	-a "org.opencontainers.image.title=Conglobate $VERSION" \
	-a "org.opencontainers.image.description=Conglobate build appliance for pichi" \
	-t "conglobate:$VERSION"

if [ -n "${GITHUB_ENV:-}" ]; then
	echo "CONGLOBATE_VERSION=$VERSION" >> "$GITHUB_ENV"
fi

echo ">>> imported conglobate:$VERSION (carapace + dtb + pmi + config)"
echo "    push (main CI only): $IMAGE:$VERSION and $IMAGE:latest"
