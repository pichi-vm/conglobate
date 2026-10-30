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

## Locked output contract

The 28 October 2026 decision in
[DESIGN §11](../DESIGN.md#11-artifact-and-build-annotations) drops
`inventory.cbor` entirely. Build binding facts belong on the image manifest:
commit with its hash algorithm, context path in that commit, plan and lock paths
relative to context, and an optional location hint. No execution receipts or
builder/input/output mirrors are emitted. Later attestation binds the image
digest and TEE/builder. A manifest containing scutes carries one
`dev.pichi.carapace.verity.hash` committing the final salt-linked chain root.
The boot command line uses that same root. Existing verity parameters and
per-scute salt/data-blocks annotations stay; no per-scute hashes are added.

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
| `config.json` | Launch requirements (2 GiB required, 4 GiB recommended) |
| `units/` | Installed into the rootfs and enabled |
| `conglobate.plan.yaml` | Self-host later, not used by bootstrap CI |
| `check-stub.sh` | Static contract check (no VM) |

`image/build.sh` writes `image/.build/` and a temporary `image/mkosi.extra/`.
Neither is source.
