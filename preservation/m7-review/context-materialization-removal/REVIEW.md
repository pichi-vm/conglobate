# Remove unused private filesystem checkout

The user assigned Git preparation to the environment. Source inspection found
that CapturedContext already reads verified local Git objects, owns their bytes,
and services recipe/lock/COPY reads. Pipeline::run additionally materialized and
read-only-mounted a second checkout whose only subsequent use was cleanup.

Removed that duplicate checkout, its four modules and typestate/error wrappers,
Workspace::materialize and the three associated pipeline error variants. No
clone/fetch/checkout is performed by the remaining object reader. Local repository,
commit and context-relative arguments remain; the working tree is not substituted
for verified object content. Do not describe this as a new worktree-path CLI.

Retained identity verification, SHA-1/SHA-256 and packed-object handling,
submodule/LFS rejection, confined symlink resolution, and captured bytes/modes.
Adapted the useful materialization regression into a direct captured-content test
that mutates the original working file and checks the original bytes and executable
bit. Kept trailing-symlink-separator rejection. Filesystem staging/cleanup tests
are preserved with removed source under pre-context-materialization-removal.

Full tests and doctests, strict all-target Clippy and formatting pass (raw logs).
No privileged end-to-end build was rerun for this change. M3 reopened 0/3; M2 and
M4–M6 remain 0/3. No adherence pass is inferred from these checks. The attached
workspace simplification is still pending for runtime and resolver mount users.
