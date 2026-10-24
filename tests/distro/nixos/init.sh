#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
# Produced-OS init: these mounts happen at boot, never in a build command.
set -eu
read -r system < /nixos-system
for runtime in /nix/store/*-conglobate-boot-tools; do
    test -x "$runtime/bin/mount"
    export PATH="$runtime/bin"
done
# Corium retains proc/sys/dev, but supplies no writable runtime state.
for directory in /etc /var /root /bin /usr /tmp /run; do
    mount -t tmpfs tmpfs "$directory"
done
chmod 1777 /tmp
exec "$system/init"
