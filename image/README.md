<!--
SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
SPDX-License-Identifier: Apache-2.0
-->

# Build-appliance image

Bootstrap CI builds this image with mkosi from Fedora 43 packages and, on
`main`, publishes it to GHCR. Self-host via `pichi build` is later.
`conglobate.plan.yaml` is only that skeleton; it is not the bootstrap
builder.

## Registry

- Image: `ghcr.io/pichi-vm/conglobate`
- Tags pushed on main: `:0.2.0` and `:latest`
- Durable pin: `:0.2.0` (and the index digest after push). Do not rely on
  `:latest` alone.
- Index: `platform.os=pichi`, architectures `amd64` and `arm64`
- Annotations: `org.opencontainers.image.{source,revision,version,title,description}`
- Pull requests build both architectures and do not push. The index job runs
  only when `github.ref` is `refs/heads/main` and the event is not
  `pull_request`.

The rootfs is built directly from packages. Its module tree includes the
filesystems and device drivers needed by both the appliance and build stages.

## Guest

- systemd is PID 1 (`/sbin/init` after the Corium pivot).
- `virtiofs` and `fuse` stay in the rootfs module tree. They are loaded after
  pivot (`/usr/lib/modules-load.d/pichi-virtiofs.conf`), not from the initramfs.
  Units mount:
  - tag `repo` → `/run/pichi/repo` (`Type=virtiofs`, read-only)
  - tag `cache` → `/run/pichi/cache` (`Type=virtiofs`, read-only)
  - tag `output` → `/run/pichi/output` (`Type=virtiofs`, read-write)
- `/usr/bin/conglobate` is the release binary. `conglobate-build.service` is a
  oneshot whose `ExecStart` is `conglobate appliance`. When that process exits,
  success or failure, the unit powers the VM off. Appliance stdout and stderr
  go to the journal and to the kernel console (`console=hvc0`).
- `e2fsprogs` and `dosfstools` provide image-resident `mke2fs` and `mkfs.fat`.
  The engine captures each formatter and its `ldd` dependencies from the
  appliance. Loop, FAT and its character-set modules remain available.
- `systemd-networkd` configures the virtual NIC with DHCP. The build service
  runs in a separate offline namespace; only `network: true` commands join
  the configured appliance namespace. `systemd-resolved` supplies its DNS
  configuration through `/run/pichi/resolv.conf`.

Image hardcodes, not CBOR fields: UID/GID maps, the network-namespace path, and
the resolver path (`src/appliance/mounts.rs`).

Build-port command version 1: `conglobate appliance` opens `dev.pichi.build`, writes
`["build", 1, null]`, then reads one self-delimiting host response.

## Locked output contract

The 28 October 2026 decision in
[DESIGN §11](../DESIGN.md#11-artifact-and-build-annotations) drops
`inventory.cbor` entirely. Build binding facts belong on the image manifest:
commit with its hash algorithm, context path in that commit, plan and lock paths
relative to context, and an optional location hint. No execution receipts or
builder/input/output mirrors are emitted. Later attestation binds the image
digest and TEE/builder. A manifest containing scutes carries one
`dev.pichi.carapace.verity.hash` committing the final salt-linked chain root.
The builder reads that commitment once through `pichi inspect` and uses it
for `corium.root` in native bootconfig. Missing, malformed and zero roots are rejected.
It requires a Pichi producer that records this manifest commitment. Existing
verity parameters and per-scute salt/data-blocks annotations stay; no
per-scute hashes are added.

## Disk and boot components

The carapace contains the complete mkosi GPT disk, using 4096-byte logical
sectors and the native Discoverable Partitions Specification root type.
Corium maps only that partition through its read-only root view. The split
ext4 artifact is used to inspect filesystem geometry during packaging.

Arma embeds the base platform device tree in the PMI and measures only the
fixed `bootconfig` kernel command line. Native configuration carries
`corium.root` and `kernel.console` / `kernel.systemd.*` settings. Pichi imports
it with `--bootconfig` and preserves the PMI bytes. There is no detached DTB.
A Corium binary update requires rebuilding the PMI; a new image root requires
only rebinding its existing `corium.root` configuration value.

## OCI output

The engine publishes only `oci-layout`, `index.json`, and `blobs/sha256/…`
into the empty `/run/pichi/output` mount (move each child, no replacement).
Every referenced inherited and produced blob is included, with the locked
build-context annotations on the image manifest.

[Conglobate #24](https://github.com/pichi-vm/conglobate/issues/24) removes
`build.yaml` and flat `NNNN.cow` / `NNNN.verity` / `boot.pmi` copies after
host OCI import landed in
[Pichi #28](https://github.com/pichi-vm/pichi/pull/28) (`7df44442`). The host
imports the full OCI graph and rebuilds verity from COW blobs and per-scute
salts. Firmware remains in its OCI blob paths.

## Tree

| Path | Role |
| --- | --- |
| `mkosi.conf`, `mkosi.finalize`, `mkosi.repart/` | Fedora 43 rootfs |
| `build.sh` | Corium initramfs, `pichi import`, local tag `conglobate:0.2.0` |
| `config.json` | Launch requirements (2 GiB required, 3 GiB recommended) |
| `units/` | Installed into the rootfs and enabled |
| `conglobate.plan.yaml` | Self-host later, not used by bootstrap CI |
| `check-stub.sh` | Static contract check (no VM) |

`image/build.sh` writes `image/.build/` and a temporary `image/mkosi.extra/`.
Neither is source.

Input scutes are prepared by Pichi in the read-only cache.
Conglobate discovers them in manifest order from its trusted tip and delegates
header validation and lazy block verification to Carapace. The encoded OCI
cache supplies metadata and blobs for output copies, not a decompression path.
