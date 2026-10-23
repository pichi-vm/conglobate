# Hosted lifecycle retry

Run 40038660026 at 5d95d69. KVM device ownership remains runner/0600 across three launches. cache-storage and contained both pass their full assessments; contained verifies launch failure, join failure, pre-admission descendant drain and abrupt helper-death drain. The udev correction is therefore supported for these launches, not every possible host interference.

runtime-scope boots and invokes the actual fixture, but Runtime::create returns EINVAL before its first callback succeeds. Kernel is Ubuntu 6.8.0-147-generic. This is a real portability observation, not an absent-KVM failure. clone-filter was not reached because the workflow stops on failure. No production fix is inferred yet. Added an optional --trace-mounts runner mode that stages trusted strace and traces mount/pidfd/statx syscalls only inside the guest. CI diagnostic retry enables this flag. Tracing is observer instrumentation, not uninstrumented qualification.

All 157 selected source files in each of three guest source archives match commit 5d95d69. Raw logs, statuses, inputs and source are retained. Large kernel/initramfs copies remain local with hashes and finite hosted retention, not durable M8 archives. No milestone pass or full hosted success is claimed.
