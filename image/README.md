<!--
SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
SPDX-License-Identifier: Apache-2.0
-->

# Build-appliance image

Bootstrap CI builds this image with mkosi from Fedora 43 packages and, on
`main`, publishes it to GHCR. Self-host via `pichi build` is later.
`conglobate.recipe.yaml` is only that skeleton; it is not the bootstrap
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

The rootfs is not a layer on `ghcr.io/pichi-vm/fedora`. That carapace does not
treat virtiofs as first-class, so this image keeps the modules itself.

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
- `e2fsprogs` provides image-resident `mke2fs` (standard paths, `/usr/sbin/mke2fs`
  first). The engine still captures that binary and its `ldd` dependencies from
  the guest root. There is no host formatter path.

Image hardcodes, not CBOR fields: UID/GID maps, the network-namespace path, and
the resolver path (`src/appliance/mounts.rs`).

v0 build-port: `conglobate appliance` opens `dev.pichi.build`, writes
`{"v":1,"op":"build-input"}`, then reads one self-delimiting host response.

## Dual-write output

The engine publishes into an empty `/run/pichi/output` (move each child, no
replace):

1. OCI layout: `oci-layout`, `index.json`, `blobs/`, `inventory.cbor`.
2. Transitional flat files for the current host:
   - `build.yaml` matching `BuildOutput` / `ScuteOut`
   - delta scutes only, chain order base→top: `0000.cow`, `0000.verity`, …
   - `boot.pmi` when this build produced firmware

Inherited base scutes are not repeated. The host prepends the operator source
carapace from `from:`. An empty `scutes` list is valid. The salt is the full
scute salt hex.

## Tree

| Path | Role |
| --- | --- |
| `mkosi.conf`, `mkosi.finalize`, `mkosi.repart/` | Fedora 43 rootfs |
| `build.sh` | Corium initramfs, `pichi import`, local tag `conglobate:0.2.0` |
| `config.json` | Launch requirements (2 GiB required, 4 GiB recommended) |
| `units/` | Installed into the rootfs and enabled |
| `conglobate.recipe.yaml` | Self-host later, not used by bootstrap CI |
| `check-stub.sh` | Static contract check (no VM) |

`image/build.sh` writes `image/.build/` and a temporary `image/mkosi.extra/`.
Neither is source.
