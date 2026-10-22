Entry ordering: read-only canary fixture
======================================

This fixture runs only in the disposable guest. Its image-side program reads
known test strings and checks descriptor/environment visibility. It does not
modify supervisor state, seek credentials, exploit the kernel, or contact an
external service. Neither VM case exposes host disks or directories.

The negative control reproduces the old prototype's sequence: pivot_root, then
execute the image's interpreter before old-root detach and FD closure. The
instrumented interpreter sees the old-root canary and inherited descriptor9.

The corrected case starts the trusted Rust helper before pivot_root. The
helper enumerates inherited FDs in trusted procfs, pivots, resets cwd, detaches
old root, closes FDs>=3, and execs the image program with an explicit environment.
The image program verifies the old-root canary is inaccessible, FD9 is closed,
cwd is /, and the synthetic EVIL_SETUP variable was not inherited.

The helper compiles with forbid(unsafe_code) using nix safe APIs. This establishes
feasibility of the ordering; it is not the complete production launcher. Keyring,
capability, syscall-filter, stdio, signal and resource policies need their own
validation. This helper assumes one thread and a trusted pre-pivot proc mount.

Reproduce from repo root:
 python3 validation/sandbox/guest.py entry --kernel-version 7.2.10-200.fc44.x86_64 --out /tmp/new-entry-fixture --run

Recorded result: reproduced/entry relative to validation/sandbox.
An initial fixture omitted /bin from inherited PATH; a subsequent diagnostic
also discovered that redirecting stderr to a missing /dev/null made an FD check
ambiguous. The retained final fixture uses shell builtins and no such redirection;
it observes the expected Bad file descriptor error for FD9 directly.
