#!/bin/bash
set -eux
touch /run/global-label-fixture
mount -t bpf bpf /sys/fs/bpf
/label-loader

modprobe virtio_pci
modprobe virtio_net
modprobe virtio_blk
for module in loop ext4 dm-zero dm-snapshot dm-verity veth nf_tables nft_chain_nat nft_masq nft_ct; do modprobe "$module"; done
ip link set lo up
# Exactly one supplied virtio NIC; use its kernel name rather than naming policy.
interface=
for candidate in /sys/class/net/*; do
    test "${candidate##*/}" = lo && continue
    test -z "$interface"
    interface=${candidate##*/}
done
test -n "$interface"
ip link set "$interface" up
ip address add 10.0.2.15/24 dev "$interface"
ip route add default via 10.0.2.2
echo 1 > /proc/sys/net/ipv4/ip_forward
echo 1 > /proc/sys/net/ipv6/conf/all/forwarding
mkfifo /work/network-capture-ready
/network-fixture network-capture "$interface" 39835 > /work/network-capture-ready &
network_capture=$!
read -r ready < /work/network-capture-ready
test "$ready" = NETWORK_CAPTURE_READY
rm /work/network-capture-ready

if conglobate build --repository /context --commit 93a9ecb50266d81b97ccc1e42583f2d1c2d9198f --recipe recipe.yaml --lock images.lock --cache /cache --output /work/output --resolv-conf /etc/resolv.conf --formatter-runtime /formatter.json; then
    :
    wait "$network_capture"
for link in /sys/class/net/cgb*; do test ! -e "$link"; done
nft list tables > /work/network-tables
test -z "$(grep conglobate_ /work/network-tables)"
echo NETWORK_BUILD_RESOURCE_CLEANUP_PASS
else
    status=$?
    :
    wait "$network_capture"
for link in /sys/class/net/cgb*; do test ! -e "$link"; done
nft list tables > /work/network-tables
test -z "$(grep conglobate_ /work/network-tables)"
echo NETWORK_BUILD_RESOURCE_CLEANUP_PASS
    test ! -e /work/output
    echo FAILED_BUILD_OUTPUT_ABSENT
    exit "$status"
fi
# Only the trusted VM supervisor sees this export device. The build workspace
# remains RAM-backed; commands receive neither the device nor an export path.
tar -C /work/output -cf /dev/vda .
sync
echo DISTRO_BUILD_AND_EXPORT_PASS
