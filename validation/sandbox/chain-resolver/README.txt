Resolver injection through actual carapace/FUSE/sealing
======================================================

Reproduce:
  python3 validation/sandbox/guest.py chain-resolver \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-CHAIN-RESOLVER --run

The real committed/locked Chain/Snapshot pipeline mounts a writable layer with
the mapped FUSE worker. The image contains a regular resolver file and an
absolute /etc/resolv.conf symlink to /run/image-resolv. Trusted setup binds
captured fixture bytes read-only over both known regular targets in the
command's private mount namespace, before the restricted entry helper pivots.
The command has mapped root, seccomp and no_new_privs, and cannot reach /work,
the context, supervisor helper or FUSE device.

Observed passing sequence:
- command reads injected resolver bytes through both the regular target and
  symlink; both writes fail with Read-only file system;
- command descendants stop while the trusted worker remains alive;
- the worker sees original resolver bytes and the symlink after command exit;
- after graceful worker teardown, sealing and chain reconstruction, kernel
  read-only remount verifies original resolver bytes, 0644 root ownership and
  the exact absolute symlink target;
- unchanged base-scute checks, actual inventory generation and publication pass;
- the trusted source resolver still contains its original captured bytes.

This integrates runtime overmounting with real block-layer output. It does not
promise zero filesystem metadata/journal changes, or validate network lookup,
malicious/dangling resolver paths, races during path selection or cancellation
inside this combined fixture. The two targets are explicit known fixture paths;
no host lookup follows the image's absolute symlink. Earlier standalone
resolver-lifecycle covers cancellation gates; that evidence must not be treated
as cancellation coverage for this full chain. The worker is the experimental
finalization-patched build, not a qualified maintained backend.

Evidence: reproduced/chain-resolver (source snapshots, locked-input and guest
runtime identities, command/results and worker build catalog).

Strict selection integrated into the chain
-----------------------------------------
The current fixture replaces the known-path shell mounts above with the safe
resolver-target helper. Reproduce with the same command. Evidence:
reproduced/chain-resolver-selected. All six checks use the mounted FUSE image:
dangling link and missing/../existing return ENOENT, cycle returns ELOOP,
regular-file trailing slash returns ENOTDIR, and FIFO/device metadata fail the
regular-file requirement. No FIFO or device content operation is performed.

Before runtime proc/dev mounts or any command execution, --install selects the
image's /etc/resolv.conf and /etc/regular-resolv using openat2 confined to the
image root. It binds through pinned file descriptors and makes each covering
mount read-only. A trusted continuation then establishes runtime mounts and
executes the existing restricted entry helper. All selection/source handles
are dropped before that continuation; no new helper child retains the namespace.
The helper is not copied into the image filesystem or exposed to commands.

The restricted command sees injected bytes through regular, absolute-link and
relative-link paths; all attempted writes return read-only filesystem errors.
Command namespace destruction removes the temporary mounts. The live worker
sees original contents; graceful worker teardown, sealing, chain reconstruction
and kernel read-only remount retain original bytes/mode/ownership and exact
absolute/relative symlink targets. The original FIFO/device remain, and no
missing resolver target is created. Base hash and committed inventory/publication
checks pass. These results extend the earlier known-target integration, not its
historical captured sources.

The standalone tmpfs matrix also passes after adding --install/--reject modes.
The chain run uses namespace destruction, not the standalone matrix's explicit
detach-and-close teardown. Interruption between selection/bind/remount and
continuation, arbitrary concurrent writers, actual DNS traffic and host/VPN
resolver policy remain separate gaps. The experimental backend is unchanged;
this does not resolve its metadata limitations or qualify production execution.
