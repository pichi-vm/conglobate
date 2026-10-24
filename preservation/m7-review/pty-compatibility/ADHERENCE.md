# Nix PTY compatibility correction

NixOS v2 reached real package acquisition but its builder failed at `ptsname_r`:
TIOCGPTN was denied. Command cleanup completed and output remained absent.
Pinned upstream 2.35.3 source (src/libutil/terminal.cc and
src/libstore/unix/build/derivation-builder.cc at github.com/NixOS/nix) identifies
the required sequence: PTY number query, unlock, get attributes, set raw mode.
The local libc trace additionally proves glibc 2.43 uses TCSETS2, not TCSETS.

Add exactly TIOCGPTN, TIOCSPTLCK, TCSETS and TCSETS2 to the ioctl allowlist.
No unrestricted ioctl fallback. TIOCSTI, TIOCCONS, TIOCSCTTY and other terminal
mutations remain denied. The fixture uses safe library APIs only. v1 exposed a
fixture /dev/ptmx collision; v2 exposed TCSETS2; v3 passes PTY output transfer
and verifies that unrelated TIOCSWINSZ remains denied. These failed runs remain
preserved. Real NixOS qualification continues separately.

This small M4 correction reopens its affected gate; three focused self-reviews
below restore it, reusing the existing full lifecycle review/evidence for
unchanged behavior. They are source reviews, not three test campaigns.

1. Requirements/style/authority review: read CommandSyscalls and the Nix call
   sites. Four scalar ioctl values, no new capabilities or namespace access,
   no freestanding production functions, no unsafe Rust, no state/lifetime
   changes. Existing default-deny and architecture guards remain. Clean.
2. Ownership/error-path review: trace CommandDevices -> private newinstance
   devpts -> EnteredRoot::enter -> receiver piped stdout/stderr -> diagnostic
   relay. No host terminal descriptor reaches the command; setsid disconnects
   the old controlling terminal, and image devices remain nodev. The new calls
   act on command-owned terminals. Filter installation failure still rejects
   admission; PTY handles remain RAII-owned in tests. Clean.
3. Evidence/scope review: inspect failed Nix run, libc trace, final fixture
   source/report and source hashes. Positive PTY byte-transfer and negative
   unrelated ioctl assertion pass in a disposable VM. No broader compatibility,
   architecture, confidential-platform or completed NixOS claim is inferred.
   Existing M4–M6 invariants remain unchanged. Clean.

M4 remains 3/3 after this bounded correction. M7 remains active. User explicitly
deferred confidential-computing qualification; Arm64 CI is now part of the
remaining gate. No legacy/evidence retirement has occurred.
