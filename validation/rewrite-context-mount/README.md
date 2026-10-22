# Read-only context view probe

Safe Rust probe, executed only in a disposable QEMU/KVM x86-64 guest. It tests
bind/remount read-only enforcement and the retained authority of a directory FD
opened before the bind mount. No host mount is performed. The marker check is a
fixture misuse guard, not a sandbox/security boundary.

Build with cargo build --manifest-path validation/rewrite-context-mount/Cargo.toml.
Run run.py with a new output directory. The runner requires the preserved seed
/var/tmp/conglobate-cap-review-fix-normal-v1 (root and kernel), cpio, QEMU and KVM.
Full output directories are /var/tmp/conglobate-rewrite-context-mount-v1 and -v2.
Input hashes, exact VM command and serial results are retained under evidence/.

v1: probe succeeded but harness failed because shutdown output interleaved with
its exit marker. PID 1 exited before asynchronous power-off completed. Retained
as failure evidence; not counted as a successful harness run.
v2: PASS after flushing the status before synchronous guest reboot (QEMU uses
-no-reboot). Reopened directory view rejects writes with EROFS; the pre-bind FD
can create a benign fixture file; contents remain readable; explicit unmount and
cleanup succeed after closing the mounted directory FD.

This settles the API primitive, not production ownership or recovery. Production
must close pre-bind directory authority and use the reopened read-only view.
An owning transition and failure/teardown handling are still required. No claim
about full sandbox isolation, crash recovery, Arm, or confidential hardware.

v3/v4 exercise the actual conglobate library transition as well as the primitive:
private tmpfs materialization -> read_only; file fchmod is rejected with EROFS;
normal close returns an owning recovery error while a file remains open; dropping
that file permits cleanup retry; Drop without explicit close cleans up. v4 also
includes the tightened staged-reader API (borrowed, no raw descriptor escape).
These runs do not qualify simultaneous crash recovery or command containment.
