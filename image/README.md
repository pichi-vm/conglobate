<!--
SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
SPDX-License-Identifier: Apache-2.0
-->

# Build-appliance image (stub)

This tree is a **non-promotion stub** for the Conglobate build appliance.

It records the intended guest shape — systemd as PID 1, static virtiofs mounts
for tags `repo` / `cache` / `output` at `/run/pichi/{…}`, and a oneshot unit
that runs `conglobate appliance` — so reviewers can see the contract without a
published OCI image.

**Not claimed here**

- GHCR push, multi-arch index, or promotion (Follow-up A)
- Self-host construction via `pichi build` (later)
- A bootable rootfs or hypervisor wiring inside this crate

The engine reads only fixed paths and the named console; mount tags and image
hardcodes (uid/gid maps, netns, resolv.conf) live in these units / the image,
never in the appliance CBOR request.
