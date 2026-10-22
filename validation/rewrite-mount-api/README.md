# Safe mount API qualification

M4 prerequisite investigation. This separate probe forbids unsafe application
Rust and makes no host mount/namespace changes. The doctest only compiles the
API; the executable exercises it in a disposable QEMU guest as described below.

The pinned released rustix 1.1.5 lacks mount_setattr. An existing upstream
[proposal](https://github.com/bytecodealliance/rustix/pull/1630) supplies the safe
interface. Reviewed commit: `8bda151df87574f56f1e4c0b59f2d1df5d61609e`, obtained
from the upstream pull ref and pinned in this probe's Cargo files. The Git source
is authoritative for this investigation; web search/API results did not reliably
expose the pull contents. No merge/release status is inferred.

The interface uses AsFd for the mount, MountAttr with a BorrowedFd lifetime for
the user namespace, and internally computed structure size. Inspected both
linux_raw and libc backend implementations. Their wire structure uses four u64
fields and the syscall size is computed from that structure. No application
pointer cast, raw syscall, or locally disguised unsafe helper is needed.
The candidate retains rustix's Apache-2.0/MIT licensing options.

Also inspected cached libcontainer commit
`cb3214c2204a03f1b99ae7c5870138439fd84345` (Apache-2.0). Its syscall wrapper accepts
a caller-provided structure size and raw userns_fd field; its crate has a much
broader dependency surface. It is not selected for this prerequisite. This does
not assert that all uses of that library are unsafe.

Results on the current x86-64 host:

- Default linux_raw backend: library compilation and compile-fail doctest pass.
- libc backend: library compilation and the same doctest pass.
- The negative client fails with E0597 because its namespace File is dropped
  before IdmapRequest. This is the intended lifetime restriction.
- All-target/all-feature Clippy with warnings denied passes; formatting passes.
- Initial probe used EMPTY_PATH; the proposed API names it AT_EMPTY_PATH.
  That compile error was corrected before the successful results above.

Commands (TMPDIR is a host test-runtime setting):

```sh
TMPDIR=/var/tmp/conglobate-rewrite-tmp cargo test --manifest-path validation/rewrite-mount-api/Cargo.toml --locked --doc -- --nocapture
TMPDIR=/var/tmp/conglobate-rewrite-tmp cargo test --manifest-path validation/rewrite-mount-api/Cargo.toml --locked --features libc-backend --doc -- --nocapture
TMPDIR=/var/tmp/conglobate-rewrite-tmp cargo clippy --manifest-path validation/rewrite-mount-api/Cargo.toml --locked --all-targets --all-features -- -D warnings
```

## Disposable VM results

The executable and run.py create an ext4 fixture in a RAM workspace inside the
stock Fedora x86-64 guest (kernel 7.2.10-200.fc44, SELinux disabled). The fixture
uses trusted host-VM formatting and util-linux unshare to supply a user namespace;
this is a syscall qualification harness, not the production command launcher.

- v1 failed before the Rust probe because the fixture omitted modprobe loop.
  The failed serial log is retained. run.py now loads loop/ext4 explicitly and
  supplies ext4 module dependencies if the selected kernel requires them.
- v2, linux_raw backend: PASS. Detached clone and IDMAP application succeed.
  Original 0:0 appears as 100000:100000. Chown through the mapped view to
  100123:100456 produces 123:456 in the original view. After clean unmount and
  remount, 123:456 remains recorded on the filesystem.
- v3, libc backend: PASS with the same assertions.
- Both successful runs reject a regular file supplied as the user namespace,
  retain detached mount ownership after attachment to a nonexistent destination
  fails, successfully retry attachment, and check explicit unmount and Drop
  cleanup leave the destination empty.

Build with cargo build using the manifest above (add --features libc-backend for
v3), then run `python3 validation/rewrite-mount-api/run.py NEW_OUTPUT_DIRECTORY`.
Only QEMU performs the privileged fixture operations. run.py itself copies files,
packages an initramfs and invokes QEMU; host modprobe --show-depends only discovers
module files and depmod indexes the copied tree. No host module is loaded.

Evidence directories v1–v3 retain serial, input hashes, QEMU command and verdict.
Provenance records the probe-source and kernel hashes and full artifact paths.
Initial API success permits pinning this upstream revision in production, but
the production mount/resource ownership API still needs implementation and tests.
No SELinux/LSM, command isolation, crash recovery, arm64 or confidential hardware
qualification is supplied by these fixture results.
