# Alpine bootstrap and OpenRC qualification

This recipe starts with the existing Fedora tool carapace and formats a new
ext4 root directly. It extracts Alpine's pinned minirootfs into that root, then
runs APK and OpenRC configuration under Alpine's musl userspace in the carapace
stage. Firmware comes from the known local kernel/corium pair; this does not
qualify Alpine kernel generation.

Prepare a separate build context by copying this directory and adding
`alpine-minirootfs.tar.gz` with SHA-256
`c5ca053cfe1d85c5b96dff8b9bc57045f7f184a30ffb6b65776409ca90388677`.
The preserved upstream input is Alpine 3.24.2 x86_64 minirootfs. The recipe checks
the archive hash before extraction. Acquisition is external to conglobate;
the archive and selected firmware pair become committed context inputs.

Use the maintained `run.py` with the Fedora cache/pin, this context's recipe,
and `--firmware-pmi`/`--firmware-dtb`. Boot the resulting layout with `boot.py
--expect CONGLOBATE_ALPINE_OPENRC_BOOT_PASS`. A build alone is not a boot pass:
the marker must come from OpenRC's local service after checking Alpine identity,
package records and persisted root ownership. The representative amd64 build and OpenRC boot passed; evidence is in
`preservation/m7-review/alpine-build-v2`. Other kernels, architectures and
confidential platforms remain unqualified.
