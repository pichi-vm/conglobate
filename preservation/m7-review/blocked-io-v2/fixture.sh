#!/bin/bash
set -eux
modprobe loop
modprobe dm-zero
modprobe dm-snapshot
modprobe ext4
/lifecycle-tests sandbox-blocked-io
