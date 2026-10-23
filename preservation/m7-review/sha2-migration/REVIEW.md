# First-party SHA-2 migration preparation

Conglobate base b393bf2; its dependencies are unchanged in this batch.

Current conglobate lock still pulls sha2 0.10.9 through devmap-verity at 2679426 and pichi-artifact at 64feac9. The clean sibling devmap repository already contains SHA-2 migration commit 3a2728f617e9272be5224bbf76ffc4deafdfe83b, preceded by private hash-adapter isolation and SHA-3 migration. It is not a manifest-only dependency update; combined integration must be tested before selecting a new pin. Its formatting/tree writer source is unchanged across those revisions.

Pichi-artifact still used SHA-2 0.10. Created isolated worktree ../pichi-artifact-sha2-011 on codex/sha2-011, starting from clean 64feac956d4539fb79b9490a90480a81ef098ebb. Changed only Cargo.toml and necessary lock entries to SHA-2 0.11. Commit 7b97a2427f2f343a90fdd72006e3de022b519057. Original sibling working tree remains unchanged; no remote push performed.

Validation: 75 all-target tests passed, including digest::tests::from_bytes_and_from_str_agree against the fixed hello SHA256 vector; strict all-target Clippy passed; doctest command passed with zero doctests. Stable compiler, CARGO_BUILD_JOBS=2, TMPDIR=/var/tmp/conglobate-rewrite-tmp, CARGO_TARGET_DIR=/storage/conglobate-validation/artifact-sha2-target. Initial test run resolved the updated lock; subsequent Clippy/doctests used --locked. Patch and verified incremental bundle retained; bundle requires original 64feac9 commit and does not claim standalone full history.

Remaining: test combined conglobate integration, make new artifact revision available through an appropriate remote branch, update all devmap pins consistently and artifact pin, regenerate conglobate lock, run dependency policy/regressions and review affected gates. dependencies-011 remains open. No M4 pass or current-head CI claim.
