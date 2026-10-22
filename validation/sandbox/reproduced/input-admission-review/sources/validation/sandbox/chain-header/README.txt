Authenticated but invalid snapshot header matrix
================================================
Reproduce from repository root:
  python3 validation/sandbox/guest.py chain-header \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --out /tmp/NEW-chain-header --run

Disposable guest, no host disk/share/NIC. Safe-Rust helper lives at
publication/helper/src/bin/header.rs. It imports a one-block canary image,
changes exactly one header field per case, then deliberately reseals the altered
COW bytes and creates matching OCI descriptors/cache content. Actual Chain::stage
must authenticate each new root successfully, ruling out a digest mismatch as
the cause of rejection. Actual Chain::assemble then reads the header through
verity and must reject InvalidData containing 'snapshot header'.

Cases: canonical valid header, magic=0, valid=0, version=2, chunk sectors=16.
The valid control assembles and reads back the 4096-byte canary. Invalid cases
must return no usable assembled device. All five cases require no residual DM
mappings or loop attachments within a bounded cleanup interval. Harness requires
five authentication markers and five cleanup markers plus each rejection.

Evidence: reproduced/chain-header, with guest/source catalogs and pinned carapace
assembly/parser sources. The pinned assemble.rs parses ValidatedSnapshotHeader
before constructing/activating its dm-snapshot target; zero and verity mappings
may already exist, and their cleanup is covered here. This fixture does not
instrument every ioctl or imply no kernel work occurs before rejection.

Scope: four recognized header constraints on fully authenticated input. No
exception records are altered. Malformed exception lists, filesystem contents,
all decompression failure modes and generic recipe execution remain separate.
Passing this matrix does not certify hostile-image parser safety or the final
sandbox. No production code changed for these cases.
