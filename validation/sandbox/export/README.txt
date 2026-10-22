Export and inspection validation
===============================

Reproduce actual build-module tests in the owning FUSE namespace:
  python3 validation/sandbox/guest.py export \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --out /tmp/NEW-EXPORT-FIXTURE --run

The runner packages Cargo's actual conglobate library test executable, selecting
build::export_tests. It runs after the copy fixture's restricted command exits,
PID-namespace descendants terminate and the heartbeat stops. TMPDIR points into
the FUSE filesystem. The unit tests call the private output_file function and
the actual same-inode firmware check, extracted unchanged into a private helper
so that the production check can be exercised directly. No new public API.

Passing path/type cases:
- Absolute and relative links resolve within the image root; a link naming a
  synthetic outside canary does not resolve against the worker's root.
- Self-cycles hit the resolver's 40-symlink bound.
- A 64-directory path succeeds; a bounded 6000-character path fails.
- FIFO, link-to-FIFO, pathname Unix socket, directory and missing file fail
  without opening a FIFO or socket.
- Character-device metadata (/dev/null) and a guest-only unconnected block node
  are rejected without opening either device. No host block device is involved.
- Identical paths, hardlinks and symlink aliases cannot serve as both PMI and
  DTB. Two separate files with equal bytes are allowed.

Demonstrated constraints, not extra guarantees:
- output_file accepts a sparse 1GiB regular file on the 256MiB test filesystem.
  It does not read or materialize the file. There is no per-output size limit
  in this path validator; build-wide storage/resource policy remains external.
- A deterministic negative control validates a synthetic regular path, replaces
  only that fixture with a FIFO, and observes that the returned path now names
  a FIFO. It does not open the FIFO or run a race loop. Thus the returned path
  is not a sealed descriptor. Stopped writers must remain stopped throughout
  export; this test does not certify exports concurrent with image commands.
- Image export selects regular files, not an arbitrary recursive directory.
  copy_tree in artifact publication walks a supervisor-owned artifact tree;
  its isolation and publication-failure behavior belong to items 16/19.

Streaming export evidence:
  cargo test --locked --test artifacts
  All 5 tests pass, including a new real Artifact::put test. A bounded synthetic
  4MiB stream is consumed with requests <=64KiB. An EIO after 3 chunks propagates,
  the temporary partial file disappears, no completed blob exists, and the
  unfinished artifact cannot publish. Build::run propagates put errors with ?.
  This is component evidence, not complete FUSE final-flush or cancellation
  qualification.

Evidence:
  reproduced/export/{inputs.json,command.json,results.txt}
  src/build.rs::export_tests
  tests/artifacts.rs::export_streams_with_bounded_reads_and_rejects_partial_io_failure

Remaining linked gates:
  Oversized outputs can exhaust build-wide tmpfs, and later firmware/DTB parsing
  reads whole blobs. Complete failure/no-publication behavior under resource
  exhaustion remains item 17. Final writer quiescence, reliable worker flush and
  publication across cancellation remain items 14–16. The current production
  executor is not the candidate sandbox, so these fixtures do not certify it.
