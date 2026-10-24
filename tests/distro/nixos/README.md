# NixOS closure qualification

The bootstrap stage uses Fedora tools to install a pinned Nix distribution and
evaluate pinned nixpkgs, then copies a real NixOS system closure into a separately
formatted root. The carapace stage writes a numeric-ownership marker. A boot
wrapper supplies volatile writable state before NixOS activation; the root and
store stay measured and read-only. The systemd service verifies NixOS activation
and the persisted marker. Firmware is the supplied known kernel/corium pair;
this recipe does not compile a kernel.

Copy this directory into a context and add `nix.tar.xz` (Nix 2.35.3 x86_64) and
`nixpkgs.tar.xz` (NixOS 26.05 revision cd4ea8d57de16f4810bc5f14207bac45fbf19f67).
The exact hashes are checked in build.sh. Downloads are environment preparation;
conglobate only consumes local image/cache and committed context inputs.
The network-enabled Nix command can acquire closure dependencies.

Nix's own sandbox and nested syscall-filter installation are disabled because conglobate already confines the command
and deliberately denies nested mount/namespace creation. This does not disable
conglobate's seccomp, user namespace, capability, filesystem or lifecycle limits.
Use run.py with `--memory 16384` and both firmware inputs. Success requires the
CONGLOBATE_NIXOS_SYSTEMD_BOOT_PASS marker from boot.py, not merely a built closure.

The native amd64 build and systemd boot passed in nixos-build-v6. The marker
checks activation and persisted numeric ownership. Earlier failed attempts
remain preserved; nested Nix sandbox/filter settings and local unsigned-output
copy are intentional recipe choices, not conglobate confinement exceptions.
