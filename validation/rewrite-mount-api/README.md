# Safe mount API qualification

M4 prerequisite investigation; no production dependency change or runtime mount
qualification yet. This separate probe forbids unsafe application Rust and makes
no host mount/namespace changes. The public apply method is compiled, not invoked
by these tests.

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

Before production adoption, use a disposable VM to create an ext4 fixture owned
by the supervisor, clone a detached mount, apply the user-namespace idmap, attach
it privately, and verify visible and persisted UID/GID values. Test invalid
namespace/descriptor inputs and owned cleanup on failed attachment. Keep the
user namespace and original superblock owned until teardown. No SELinux/LSM,
command isolation, crash recovery, arm64 or confidential hardware qualification
is supplied by this compile-only probe.
