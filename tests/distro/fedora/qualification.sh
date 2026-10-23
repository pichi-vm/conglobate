#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
set -eu
test "$(getenforce)" = Enforcing
matchpathcon -V /etc/conglobate-qualified
test "$(stat -c %u /etc/conglobate-qualified)" = 0
rpm -q policycoreutils selinux-policy-targeted
sestatus
echo CONGLOBATE_FEDORA_ENFORCING_BOOT_PASS
