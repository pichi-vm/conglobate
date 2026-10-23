# Debian bootstrap investigation

This is preparation, not a successful Debian build or boot.

The current upstream debootstrap functions were inspected read-only. setup_devices
tries device creation and falls back to bind mounts; setup_proc also contains mount
operations. Those operations are outside the command sandbox's contract. Do not
add SYS_ADMIN or persistent device creation to make a distro helper succeed.
Source URL and exact downloaded-file hash are in checks.json. Reference:
https://salsa.debian.org/installer-team/debootstrap/-/raw/master/functions

Debian's mmdebstrap documents extract/chrootless variants and explicit mount skips;
those are candidates once a Debian tool carapace is available, not validated here:
https://manpages.debian.org/trixie/mmdebstrap/mmdebstrap.1.en.html

Initial tool-carapace construction is allowed outside conglobate. An official
Debian trixie-slim amd64 OCI filesystem was fetched with skopeo for this purpose;
its manifest/config/layer digests and lengths were verified. The one-layer archive
has no whiteouts, absolute/parent-traversal entries or device nodes. Numeric owners
are retained in the archive and recorded in oci-source.json. No image executable
was run. The tag is historical discovery input; subsequent seed construction must
use the recorded content, not re-resolve it silently.

A regular-file mke2fs -d tar probe failed because the installed trusted formatter
was built without libarchive support. The failure log is preserved. No host mounts
were performed. The documented tar feature is conditional on that build option:
https://www.man7.org/linux/man-pages/man8/mke2fs.8.html

Next: create the initial seed filesystem in a disposable VM, preserving numeric
ownership, then convert/import it outside conglobate and pin its carapace index.
Only then supply that local cache to conglobate. This is not permission for the
binary to fetch missing images. Real Debian commands and boot remain untested.

The distro runner now supplies an explicit trusted formatter runtime manifest.
Every selected executable/loader/library path was checked against the previously
constructed guest. Actual bootstrap use of this runner change remains untested.
