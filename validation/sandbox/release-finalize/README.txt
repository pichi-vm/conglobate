File-release error propagation and build rejection
===================================================
2026-10-21 UTC. Diagnostic only; no installed backend/production changes.
Reproduce worker (new output directory):
  python3 validation/sandbox/release-finalize/build.py \
    --pair /tmp/conglobate-clean-pair-v2 --out /tmp/NEW-release-worker
Backend matrix:
  python3 validation/sandbox/guest.py release-finalize \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-clean-pair-v2 \
    --protocol-worker-build /tmp/NEW-release-worker --out /tmp/NEW-release --run
Integrated rejection:
  python3 validation/sandbox/guest.py chain-release-error \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --protocol-worker-build /tmp/NEW-release-worker --out /tmp/NEW-chain-release --run

Source-grounded boundary
------------------------
The pinned patched fuse2fs op_release invokes ext2fs_flush2 with
EXT2_FLAG_FLUSH_NO_SYNC and passes failures through translate_error. Its destroy
and fsync callbacks use flags0. The diagnostic link wrapper calls the real flush
first and, only on its first successful NO_SYNC call, returns EIO. The paired
worker's sticky error flag is therefore exercised on file release independently
of actual media failure or a final close failure. Original build objects and
libraries are hashed before/after relink and left unchanged. Wrapper and link
inputs are retained; native C is diagnostic code, not unsafe Rust adoption.

Observed results
----------------
Backend control and fault each write an owned file and return success to the
shell. Clean control exits0; injected worker exits32 after unmount. Kernel
read-only remount confirms expected file contents in both cases. No fault is
injected into disk bytes. This is error propagation evidence, not a simulated
lost write or proof that a faulted filesystem is eligible for output.

The first integrated command made several writes. Its initial release error was
followed by EROFS on a subsequent write; worker exited32 and the command failed.
No output was accepted and cleanup passed. The runner expected command success
and rejected that run; retained as chain-release-error-early. The final command
makes one last heartbeat write and then only prints its completion marker. It
returns success; descendant-quiescence checks pass, worker then exits32 and the
helper rejects the build before Snapshot::finish/sealing/publication. Verified
output and published paths are absent; base bytes and supervisor mountinfo are
unchanged; FUSE connections, DM mappings and loops are released. Guest exits0
because the expected rejection/cleanup assertions pass.

Harness issues kept separate from results
----------------------------------------
An initial CLI attempt lacked the new case in the protocol-worker argument
allowlist; fixed before packaging. Initial integrated packaging then hit the
host tmpfs user quota before a guest started. Redundant generated images from
five completed runs were removed only after exact comparison of their retained
inputs/results/command files. Sources/logs/manifests remain retained; cleanup
list is in build/generated-artifact-cleanup.json. The new output directory retry
ran normally. Neither event is backend or sandbox failure evidence.

Scope
-----
This closes the tested last-write release-error-to-no-publication path for the
experimental pair. A successful command and later successful media close cannot
cancel the worker's sticky failure. Not all callback error classes, real late
write-loss/cache conditions, aborts, completion acknowledgment or coordinator
failure boundaries are qualified. Expected permission/absence/request errors
are not automatically fatal filesystem errors. Maintained error-contract/API
ownership remains open. No claim about production executor security is made.
