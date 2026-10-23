#!/bin/bash
set -eux
mkdir /work/cgroups
mount -t cgroup2 cgroup2 /work/cgroups
/lifecycle-tests domain-publication
