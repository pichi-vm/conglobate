# Background-child cleanup regression

Run this recipe through `tests/distro/run.py`. It checks that named pipes are
supported while persistent character/block device creation remains denied.

The child opens its FIFO for reading/writing before a second FIFO acknowledges
readiness to the parent. The parent exits without waiting. On the next step,
nonblocking writer-open must fail with ENXIO, proving that reader is gone; no
child output may exist. Read/write mode keeps the child's own writer open, so
it cannot voluntarily finish from EOF. No sleep determines readiness or cleanup.

Require both exact-line BACKGROUND_CHILD_STARTED_PASS and
BACKGROUND_CHILD_DRAINED_PASS markers, successful trusted status.raw and verified
OCI export. This tests a concrete background-child case, not every descendant,
helper-failure, syscall or platform scenario. Failed attempts remain preserved.
