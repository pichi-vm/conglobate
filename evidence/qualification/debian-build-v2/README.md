# Debian representative build and boot v2

Build/export passed. The boot observed CONGLOBATE_DEBIAN_BOOT_PASS after
checking Debian identity, root ownership of the build marker, and installed
systemd-sysv, udev, dbus and util-linux package records. The factory database
copy restored by tmpfiles resolves the v1 volatile-/var failure.

This is native amd64/KVM with the previously built Fedora kernel/corium
firmware pair, not Debian kernel generation, confidential-computing evidence,
or complete distro qualification. The runner terminates the VM after the
sentinel; this does not prove guest-initiated shutdown.

The external artifact and VM runtime remain at
/var/tmp/conglobate-debian-build-v2 and must be preserved before M8 retirement.
