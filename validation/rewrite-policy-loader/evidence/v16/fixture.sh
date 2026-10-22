#!/bin/bash
set -eux
export PATH=/usr/bin:/usr/sbin
mkdir -p /work/origin /work/mapped
modprobe loop
modprobe ext4
truncate -s 64M /work/volume.ext4
mke2fs -q -t ext4 -F /work/volume.ext4
mount -o loop /work/volume.ext4 /work/origin
unshare --user --map-users=0:100000:65536 --map-groups=0:100000:65536 sleep 60 &
holder=$!
echo "$holder" > /work/userns-pid
for attempt in {1..50}; do
    if grep -q 100000 /proc/$holder/uid_map; then break; fi
    sleep 0.1
done
cat /proc/$holder/uid_map
cat /proc/$holder/gid_map
mkdir /work/pins
mount -t bpf bpf /work/pins
mkdir /work/cgroups
mount -t cgroup2 cgroup2 /work/cgroups
/idmap-probe
umount /work/cgroups
umount /work/pins
if kill -0 "$holder" 2>/dev/null; then kill "$holder"; fi
wait "$holder" || true
umount /work/origin
echo POLICY_FIXTURE_CLEANUP_PASS
