#!/bin/bash
set -eux
modprobe loop
modprobe dm-zero
modprobe dm-snapshot
/lifecycle-tests formatter-failures
