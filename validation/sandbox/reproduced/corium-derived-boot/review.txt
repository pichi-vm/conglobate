Restricted derivation followed by two-scute corium boot
======================================================
Purpose: connect real restricted command writes, snapshot/FUSE finalization and
conglobate sealing to a subsequent corium boot. No offline filesystem mutation
substitutes for the recipe. No host filesystem mount, NIC or host share is used.

Preparation:
 python3 validation/sandbox/guest.py chain-acl --kernel-version 7.2.10-200.fc44.x86_64 --out NEW-TEMPLATE --protocol-worker-build /tmp/conglobate-statx-worker-v2 --worker-build /tmp/conglobate-finalize-workers
 cargo build --locked --manifest-path validation/sandbox/publication/helper/Cargo.toml --bin derive-root
 python3 validation/sandbox/corium-derived/derive.py --template NEW-TEMPLATE --base /tmp/conglobate-corium-root-v3 --helper validation/sandbox/publication/helper/target/debug/derive-root --out NEW-DERIVATION
 python3 validation/sandbox/corium-derived/boot.py --base /tmp/conglobate-corium-root-v3 --derived NEW-DERIVATION --raw-boot /tmp/conglobate-fedora-root-boot-v2 --out NEW-BOOT
The template is only packaged, not executed as chain-acl; the custom guest has
its own final input inventory and generated scripts. Use new output directories.
Large inputs are the previously qualified bootstrap-output/corium fixture.

The safe Rust helper opens the read-only GPT partitions, authenticates their
base root through carapace::Assemble, creates conglobate::Snapshot, and grants
the mapped worker access to the resulting writable block device. Worker runs
paired ACL/statx/finalization FUSE. A separate mount/PID/IPC/UTS/network namespace
and the candidate restricted entry helper execute the recipe. The command
checks NoNewPrivs/seccomp, hidden supervisor/context/device paths, and inherited
Fedora identity before adding a small derived marker and modifying the existing
root boot probe. This is a fixed diagnostic recipe, not the generic executor.

Trusted worker injects/removes /command and verifies the existing empty .old
mountpoint without following a symlink. Shell array/glob checks avoid external
utilities. Worker exits and FUSE unmount completes before snapshot.finish seals
the derived layer. Guest requires no remaining DM mappings/attached loop devices
before export. A safe Rust helper exports only three bounded owned files through
the named virtio port; the host verifies lengths/hashes and exact EOF. The framing
is diagnostic, not a new conglobate inventory format. Each file is bounded128MiB.

The second guest receives a fresh GPT containing both unchanged base partitions
plus derived data/hash partitions. Its corium root argument is the derived hash.
Static corium and matching7.2.11 kernel/modules are the prior verified artifacts.
The new probe validates derived content and removal of /command before invoking
the inherited Fedora root/systemd/package/device checks. Both exact markers occur
once. The boot completes/powers off at about4.73s. Base disk, derived files and
combined disk are hashed unchanged after execution. Build guest exit0 at1.05s.

This closes the representative restricted-derived-layer/corium/root-switch gate.
It does not qualify arbitrary depth, OCI transport, generic recipes, firmware/DTB
integration, distribution-version upgrade, target SELinux enforcement, all-service
health or the final sandbox. Login/home service failures and busy DM mappings at
systemd shutdown remain visible, as in the single-scute case. Poweroff is not
proof of successful DM teardown in a continuing kernel. Backend special-metadata
limitations, identity allocation and final lifetime integration remain blockers.

Failed preparations/runs retained, never counted as passes:
* Initial template lacked required --worker-build and stopped during packaging.
* Derivation v1 rejected an already existing empty .old before running commands.
* v2 ran/sealed, but export lacked Python. Its ls-based emptiness check also
  incorrectly treated missing ls as empty; this is corrected with shell arrays.
* v3 ran/sealed, but assumed vport0p1. Export now discovers the trusted port by
  its configured name; attached block devices affect virtio device numbering.
* v4 has all required success markers and guest exit0; second boot also passes.
Sources, generated scripts, input catalogs, commands, complete logs, output
identities and cleanup notes are retained under reproduced/corium-derived-*.
Redundant base scute files were removed only after verifying identical bytes at
recorded offsets in the retained base GPT. Failed/completed generated guest roots
were removed after preserving logs, input inventories and generated scripts.
