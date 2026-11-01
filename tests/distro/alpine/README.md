# Alpine bootstrap and OpenRC qualification

This plan starts with the existing Fedora tool carapace and formats a new GPT disk
with an ext4 root. It extracts Alpine's pinned minirootfs into that root, then
runs APK and OpenRC configuration under Alpine's musl userspace in the carapace
stage. Firmware comes from the supplied embedded-DTB PMI; this does not
qualify Alpine kernel generation.

Prepare a separate build context by copying this directory and adding
`alpine-minirootfs.tar.gz` with SHA-256
`c5ca053cfe1d85c5b96dff8b9bc57045f7f184a30ffb6b65776409ca90388677`.
The required upstream input is Alpine 3.24.2 x86_64 minirootfs. The plan checks
the archive hash before extraction. Acquisition is external to conglobate;
the archive and selected PMI become committed context inputs.

Use the maintained `run.py` with the Fedora cache/pin, this context's plan,
and `--firmware-pmi`. Boot the resulting layout with `boot.py
--expect CONGLOBATE_ALPINE_OPENRC_BOOT_PASS`. A build alone is not a boot pass:
the marker must come from OpenRC's local service after checking Alpine identity,
package records and persisted root ownership. This fixture targets amd64; other
architectures and confidential platforms require separate validation.
