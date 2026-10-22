#!/bin/bash
set -eu
export PATH=/usr/bin:/usr/sbin
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev
mount --make-rprivate /
mkdir -p /work
mount -t tmpfs tmpfs /work
set +e
bash /fixture.sh
status=$?
echo "IDMAP_PROBE_EXIT $status"
echo VM_CONTROLLER_POWER_OFF
echo o > /proc/sysrq-trigger
exec sleep infinity
