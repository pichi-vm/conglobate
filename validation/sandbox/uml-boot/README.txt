Debian packaged UML boot timing — 2026-10-21
==========================================
Result: unmodified Debian stable UML boots a minimal BusyBox initramfs as
host UID1000. Final series: ten successful boots, exact readiness/completion
markers, verified capability-only active LSM list, clean poweroff and exit0.

Inputs
------
Debian trixie amd64 user-mode-linux 6.12um1+b21; runtime kernel6.12.101.
Debian busybox-static 1:1.37.0-6+b9. Packages were downloaded, not installed.
Debian InRelease signature verified with the installed archive keyring;
Packages.xz SHA256/size verified against signed metadata; both .deb SHA256s
and sizes verified against that index. Reproduction rechecks .deb hashes.
Raw packages and full index: /var/tmp/conglobate-debian-uml-v1.
Exact identities, signed metadata, signature output, kernel config, script
snapshots, logs and results: ../reproduced/debian-uml-boot, indexed by files.json.

Measurement
-----------
Host time.monotonic_ns immediately before subprocess.Popen to receipt of the
exact UML_BOOT_READY line. PID1 checks its PID and mounts proc before emitting
that line. Includes executable launch, UML host checks, kernel initialization,
initramfs unpacking, shell startup, proc mount and console delivery. Excludes
package downloads/extraction and archive preparation. Not pure kernel-only time.
After readiness: uname, uptime, sysfs/securityfs mounts, active-LSM assertion,
PID1 status, completion marker and forced guest poweroff.

Final v3 series, seconds:
                              Median        Min         Max
Launch -> readiness           0.949877      0.939469    1.005623
Launch -> process reaped       1.013632      0.999382    1.070280
Ten sequential fresh UML instances; no host-cache flushing, CPU pinning or
isolated benchmark host. These are repeated/warm-cache observations, not a
controlled cold-boot benchmark. Host AMD EPYC8324P32-core; x86_64 kernel
7.2.11-dm-bufio-test1-dirty; host SELinux Enforcing, unchanged.
Requested UML RAM128MiB with temporary memory backing in a private /dev/shm
directory. UML additionally reserves an executable-layout gap; requested RAM
is not a measurement of total RSS. Stock kernel reports one CPU.

Important configuration finding
-------------------------------
The first boot (v1, readiness0.942660s) and ten v2 repeats used selinux=0.
Those boots succeeded, but this did NOT disable SELinux: the package config
has CONFIG_SECURITY_SELINUX=y and CONFIG_SECURITY_SELINUX_BOOTPARAM unset.
Logs show SELinux initialization and identify selinux=0 as an unknown option.
The final runner uses lsm=capability; every v3 run reads securityfs and requires
the active LSM list to equal capability. No policy is loaded. This qualifies
LSM selection on this package, not independent-image labeling or enforcement.
v1/v2 logs are retained as boot evidence and a corrected configuration
assumption; they are not SELinux-disabled evidence.

The final series uses an owned stdin pipe instead of /dev/null to remove a
console epoll diagnostic. Initramfs-only boot still logs failure to initialize
default ubd0 because no block image was supplied. /init executes successfully
without a root disk; no backing disk or hostfs mount is used by this fixture.

Reproduce
---------
python3 validation/sandbox/uml-boot/fetch.py --out NEW-PACKAGE-DIRECTORY
python3 validation/sandbox/uml-boot/run.py --packages PACKAGE-DIRECTORY \
  --out NEW-RESULT-DIRECTORY --runs 10

fetch.py selects current trixie package versions, which can change. To repeat
these exact inputs, use retained packages.json Filename/SHA256/Size entries
and archived package bytes; those fields are checked by run.py. Keyring is
configurable. Requires Python3, dpkg-deb, gpgv, executable tmpfs at /dev/shm,
and a host that permits UML's ptrace/mmap operations. Every output directory
must be new. No root/sudo, package installation, host mount or kernel change.
The runner has a30-second per-instance deadline and2MiB output cap, terminates
only its own process group, reaps the launcher and removes its runtime dir.

Scope
-----
This is an authorized benign kernel boot/timing probe directly on the current
host. It is not nested inside the earlier QEMU fixtures, nor is it a security
test of UML or a prototype of the final sandbox. Stock config enables hostfs,
management console, modules and other features requiring confinement before
untrusted recipes run. Management sockets were confined to private temporary
directories; no network interface was configured. Only our fixed init executes.
Native ext4/XFS/Btrfs are built into the package, but none was mounted here.
Storage/metadata compatibility, actual carapace assembly, build workloads,
untrusted-command isolation and final publication/lifecycle remain untested
for UML. This result does not complete the original sandbox validation goal.
