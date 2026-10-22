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
sleep 1
echo b > /proc/sysrq-trigger
exec sleep infinity
