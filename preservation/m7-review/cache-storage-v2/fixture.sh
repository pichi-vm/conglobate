#!/bin/bash
set -eux
modprobe loop
modprobe dm-zero
modprobe dm-snapshot
modprobe dm-verity
modprobe ext4
/lifecycle-tests cache-storage
