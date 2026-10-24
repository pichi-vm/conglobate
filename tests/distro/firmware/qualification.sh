#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
set -eu
test "$(uname -r)" = 7.2.10-200.fc44.x86_64
test -d /sys/fs/cgroup/system.slice
printf 'CONGLOBATE_ARMA_FIRMWARE_BOOT_PASS\n'
