# Debian build v1 and failed boot qualification

Build succeeded; boot reached Debian 13 login but the qualification service
could not query the installed packages. The expected sentinel was absent.
The inherited Fedora DTB uses systemd.volatile=state (see ../fedora/build.sh),
which hides Debian's /var/lib/dpkg on boot. This explains the observed symptom;
the v2 run tests the correction rather than treating source analysis as proof.

V2 adds a factory copy of the package database and a systemd-tmpfiles C rule,
with qualification ordered after tmpfiles setup. The installed systemd
 tmpfiles.d(5) documentation specifies recursive copy from /usr/share/factory
when the source argument is omitted. No assertion was weakened.

Large artifact and VM inputs remain under /var/tmp/conglobate-debian-build-v1;
these require durable recovery before M8 retirement. The terminal boot verdict is failure: the 180-second deadline expired without
the required sentinel. The harness terminated and reaped its VM launcher.
