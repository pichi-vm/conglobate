#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
set -eu
. /etc/os-release
test "$ID" = debian
test "$(stat -c %u /etc/conglobate-qualified)" = 0
dpkg-query -W systemd-sysv udev dbus util-linux
echo CONGLOBATE_DEBIAN_BOOT_PASS
