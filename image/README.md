<!--
SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
SPDX-License-Identifier: Apache-2.0
-->

# Build-appliance image

[`pichi.yaml`](pichi.yaml) builds the appliance on the Fedora image pinned in
[`pichi.lock`](pichi.lock). A shared tools stage installs Fedora's Rust, Rust
sources and LLVM packages. Separate stages build the engine and fixed LSM loader;
the policy uses `RUSTC_BOOTSTRAP=1` for Cargo's `build-std=core` support.
Only their executables enter the appliance root. Fedora supplies the kernel,
modules and immutable PMI; the appliance adds its services, policy and formatters.

The policy and loader come from the pinned [LSM submodule](../lsm),
whose repository owns their source checks.

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

## Build

Use native Linux amd64 or ARM64 with KVM, Pichi/Dillo and enough 2 MiB hugepages
for an 8 GiB build VM. CI builds the host tools from the pinned `sources/pichi`
and `sources/dillo` submodules. From the repository root, initialize the sources
and pull the locked Fedora input:

```sh
git submodule update --init --recursive
digest=$(sed -n 's/^  "ghcr.io\/pichi-vm\/fedora:43": //p' image/pichi.lock)
pichi pull "ghcr.io/pichi-vm/fedora@$digest"
BUILD_IMAGE=ghcr.io/pichi-vm/conglobate@sha256:262f87ecd7a178a90979eebd9689a0a28c1a4d844e6572a0fea116bf782a0b6f
pichi build . --ref HEAD --plan image/pichi.yaml --lock image/pichi.lock \
  --build-image "$BUILD_IMAGE" --memory 8192 -t ghcr.io/pichi-vm/conglobate:0.2.0
```

`BUILD_IMAGE` selects the existing appliance that Pichi boots to execute this
build. The locked Fedora image supplies the stage filesystems and firmware;
the output is a new appliance.

Both native Linux CI jobs build one appliance from the locked Fedora base,
run Pichi's staged-image acceptance through it, and use it to build Fedora.
This completes the self-hosting cycle: Pichi builds Fedora, and Fedora supplies
the appliance base. Both build jobs and the engine checks gate publication;
CI retains image and staged-test evidence.

## Guest

- systemd is PID 1 (`/sbin/init` after the Corium pivot).
- Fedora supplies the kernel modules; normal device and filesystem loading
  provides the appliance drivers without a custom module-load list. Units mount:
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
- `dosfstools` provides image-resident `mkfs.fat` for FAT32 filesystems.
- Loop, FAT and character-set modules remain available for build filesystems.

Image hardcodes, not CBOR fields: UID/GID maps (`src/appliance/mounts.rs`).

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
Conglobate binds the selected root in native bootconfig while preserving the
inherited Fedora PMI. Corium mounts the native root partition. The PMI's embedded DTB
and `bootconfig` command line remain immutable. Missing, malformed and zero
roots are rejected.
It requires a Pichi producer that records this manifest commitment. Existing
verity parameters and per-scute salt/data-blocks annotations stay; no
per-scute hashes are added.

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
| `pichi.yaml` | Appliance stages, inherited firmware and launch requirements |
| `pichi.lock` | Qualified Fedora input digest |
| `units/` | Installed into the rootfs and enabled |
| `10-build.network` | DHCP for the build network |

Input scutes are prepared by Pichi in the read-only cache.
Conglobate discovers them in manifest order from its trusted tip and delegates
header validation and lazy block verification to Carapace. The encoded OCI
cache supplies metadata and blobs for output copies, not a decompression path.
