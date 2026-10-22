Artifact stream cancellation regression
=======================================

Reproduce: cargo test --locked --test artifacts

The baseline Artifact::put accepted a stream after SIGTERM had already set
its cancellation flag; the new before-read assertion failed because put
returned Ok with a 65536-byte digest/size instead of "build cancelled".
That baseline failure was observed in the interactive test output, not saved
as a standalone baseline log.

The correction checks cancellation before and after each read (including
EOF) and before persisting the private blob. Each scenario runs in a separate
subprocess because the process-global cancellation flag is irreversible.
The bounded reader raises SIGTERM to itself using the installed handler:
- before-read: no reader call is allowed;
- during-read: cancellation occurs on the first read returning data;
- after-chunk: one 64KiB chunk is written before the second read cancels;
- at-eof: cancellation occurs on a read returning EOF.
All require the cancellation error, the expected call count, an empty blob
directory and no NamedTempFile residue. All six artifact integration tests
pass; source snapshots and their hashes accompany the result.

Limitations: cooperative checks cannot interrupt a blocked read/write syscall.
This test does not finalize or publish an artifact and does not establish
publication-copy interruption, rename-race semantics, or crash durability.
A cancellation arriving after the last check can still race private blob
persistence; publication has its own cancellation gate. No production sandbox
qualification is implied.
