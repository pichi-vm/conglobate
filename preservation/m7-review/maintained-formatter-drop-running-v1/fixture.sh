#!/bin/bash
set -eux
modprobe loop
modprobe dm-zero
modprobe dm-snapshot
FORMATTER_DROP=running /lifecycle-tests formatter-failures
