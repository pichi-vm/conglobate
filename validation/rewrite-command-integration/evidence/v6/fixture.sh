#!/bin/bash
set -eux
export PATH=/usr/bin:/usr/sbin
mkdir -p /work/origin /work/mapped
modprobe loop
modprobe ext4
truncate -s 64M /work/volume.ext4
mke2fs -q -t ext4 -F /work/volume.ext4
mount -o loop /work/volume.ext4 /work/origin
cp -a /image-root/. /work/origin/
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

/idmap-probe supervisor vm-death &
supervisor=$!
for attempt in {1..100}; do
    test ! -e /work/supervisor-ready || break
    sleep 0.05
done
test -e /work/supervisor-ready
kill -KILL "$supervisor"
set +e
wait "$supervisor"
status=$?
set -e
test "$status" = 137
# The independent controller has observed supervisor loss. Confirm there are
# still contained processes: shutdown must not depend on supervisor cleanup.
populated=0
for domain in /work/cgroups/conglobate-*; do
    if grep -q 'populated 1' "$domain/cgroup.events"; then populated=1; fi
done
test "$populated" = 1
echo SUPERVISOR_DEATH_OBSERVED_WITH_LIVE_DOMAIN
# Return to PID 1, which powers off this VM. Never start another build here.
