#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
set -eu
echo '6ce3ce3703fdc241b560cba401c5d338042122fe5b1a81e07bf1a623f259eba0  /nix.tar.xz' | sha256sum -c -
echo '554098cd31950c0d555461e1f116b9c1a92436a98883de76ba1e5c31e4092a5e  /nixpkgs.tar.xz' | sha256sum -c -
mkdir -p /work /nix /nixpkgs /etc/nix
tar -xJf /nix.tar.xz -C /work
tar -xJf /nixpkgs.tar.xz -C /nixpkgs --strip-components=1
cp -a /work/nix-2.35.3-x86_64-linux/store /nix/
export PATH=/nix/store/kp71jf0isr8pix0nn2lx2x3hj3vasii5-nix-2.35.3/bin:$PATH
export NIX_SSL_CERT_FILE=/nix/store/zdl7dn1gmi1cxdw5a8hw6xsf4cz0rjmg-nss-cacert-3.123/etc/ssl/certs/ca-bundle.crt
export NIX_PATH=nixpkgs=/nixpkgs
# Conglobate supplies isolation. Nix must not create nested namespaces/mounts.
export NIX_CONFIG='build-users-group =
sandbox = false
filter-syscalls = false
max-jobs = 1
cores = 1
experimental-features = nix-command
'
nix-store --load-db < /work/nix-2.35.3-x86_64-linux/.reginfo
system=$(nix-build /nixpkgs/nixos -A system --no-out-link -I nixos-config=/configuration.nix)
runtime=$(nix-build --no-out-link --expr 'let p = import /nixpkgs {}; in p.buildEnv { name = "conglobate-boot-tools"; paths = [ p.bash p.coreutils p.util-linux ]; }')
# Locally built derivations are unsigned; this copy exports our own output.
nix copy --no-check-sigs --to 'local?root=/mnt' "$system" "$runtime"
mkdir -p /mnt/bin /mnt/usr/bin /mnt/sbin /mnt/etc /mnt/var /mnt/root /mnt/run /mnt/tmp /mnt/proc /mnt/sys /mnt/dev
ln -s "$runtime/bin/sh" /mnt/bin/sh
ln -s "$runtime/bin/chown" /mnt/bin/chown
ln -s "$runtime/bin/env" /mnt/usr/bin/env
printf '%s\n' "$system" > /mnt/nixos-system
printf 'CONGLOBATE_NIXOS_CLOSURE_BUILD_PASS\n'
