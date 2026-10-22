Strict resolver target selection and descriptor-based overmount
==============================================================

Reproduce:
  python3 validation/sandbox/guest.py resolver-target \
    --kernel-version 7.2.10-200.fc44.x86_64 --out /tmp/NEW-RESOLVER-TARGET --run

The prototype Rust helper forbids unsafe code and uses rustix openat2/fstat
and nix mount APIs. It is separate from production code. The guest runs as
initial-root on a private tmpfs with no host shares, disks or NICs. A20-second
timeout bounds the complete matrix. Rust dependencies are pinned by Cargo.lock;
the harness records build command/log, executable hash and local sources.

Selection opens the trusted image-root directory, then uses O_PATH|O_CLOEXEC
with RESOLVE_IN_ROOT|RESOLVE_NO_MAGICLINKS|RESOLVE_NO_XDEV. Absolute symlinks
remain relative to that root, and excess parent traversal clamps there.
No creation flag is used. fstat must identify a regular file before it can
be a mount target. O_PATH avoids activating FIFO/device read/write behavior.

Passing rejection cases: missing target, symlink loop, absolute link naming
an outside canary, missing/../existing, trailing slash/dot through a regular
file, directory, FIFO, Unix socket and character device. A nested bind mount
is rejected with EXDEV. A separate proc-root control rejects a real proc FD
magic link with ELOOP. No ordinary host canonicalization is used.

Passing injection cases: regular file, absolute symlink, relative symlink and
relative symlink with excess parent traversal. Trusted captured resolver bytes
are bound over the selected inode through /proc/self/fd paths, then reopened
through the root to pin the covering mount. That covering mount is made
read-only. In-root reads see injected bytes; write-open returns EROFS. After
teardown, original bytes, device/inode, mode and UID/GID match. The source and
outside canary are unchanged; the absolute symlink remains a symlink with its
original target, and the missing target was not created.

The original attempt used ordinary unmount while holding the covering O_PATH
descriptor and received EBUSY. That failed run is retained in
reproduced/resolver-target-negative. The corrected fixture detaches the mount
from its private tree and closes the covering handle before checking restore.
This is explicit mount/handle lifetime handling, not evidence that lazy unmount
alone terminates users or finishes filesystem writes. There are no image writers
in this fixture. Production still needs stopped-writer ordering, cancellation
ownership and final executor integration; namespace destruction is another
teardown mechanism that this test does not implement.

Evidence: reproduced/resolver-target. This tests kernel tmpfs, not the mapped
FUSE backend or a complete carapace chain. The earlier chain-resolver fixture
uses known targets; it must be extended before combining these claims. Actual
DNS traffic, host/VPN resolver policy, concurrent mutation and replacement
after selection are not qualified. NO_XDEV deliberately rejects targets across
runtime mounts, so selection is intended before those mounts are installed.
Unsupported openat2/flags fail; no unsafe fallback is demonstrated or proposed.

Primary reference checked2026-10-20:
https://man7.org/linux/man-pages/man2/openat2.2.html
Local safe wrapper: rustix1.1.5 src/fs/openat2.rs. IN_ROOT supplies per-lookup
root semantics; NO_MAGICLINKS and NO_XDEV restrict special traversal routes.
These kernel mechanisms supplement the observed tests, not a full path-race
or filesystem-worker correctness proof.

FUSE chain follow-up: --reject ROOT PATH ERRNO asserts a particular selection
failure. --install ROOT SOURCE PATH... -- COMMAND... selects all targets first,
installs read-only descriptor mounts, closes its handles and execs a trusted
continuation. The chain-resolver fixture now exercises these modes on mapped
FUSE, through sealing and inventory publication. See its README and evidence
in reproduced/chain-resolver-selected. This advances the earlier integration
gap; cancellation and real DNS-policy integration remain unqualified. The CLI
belongs only to this test helper, not the proposed conglobate public interface.
