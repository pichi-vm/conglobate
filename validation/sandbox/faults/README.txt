Final-flush fault injection, 2026-10-20

Run only in disposable guest. Current local reproduction:
  bash /tmp/conglobate-fuse-faults/run.sh

The supervisor creates a 64MiB ext4 image on guest tmpfs, loop device and
linear device-mapper target. Mapped-user FUSE worker mounts it and writes a
file. Supervisor replaces the target with dm-error, resumes it, then asks
worker to unmount. Repeat with a freshly formatted image and errors=panic.

Observed default: unmount status 0, daemon status 0, logged EIO during final
flush and close. Observed errors=panic: unmount status 0, daemon status 134,
logged EIO. Therefore unmount success is insufficient; default daemon status
is insufficient too. The errors=panic mode detects THIS injected flush fault.
It does not prove every close-only failure is surfaced: upstream main logs
ext2fs_close errors without changing the return status, so a reliable explicit
finalization result remains a backend acceptance requirement.

Neither run publishes an image. The test exposes failure-reporting behavior;
it does not implement production rejection. All devices disappear with guest
teardown; host devices and SELinux policy are untouched.
