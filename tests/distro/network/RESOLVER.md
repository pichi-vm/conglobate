# Resolver lifecycle fixtures

Run `resolver-lifecycle.yaml` with the distro runner's `--network-fixture`, then
run `python3 tests/distro/network/verify_resolver.py RUN_DIRECTORY`.

The plan starts with an absolute dangling resolver symlink and missing ancestor
directories. The enabled command checks read-only injection and an actual DNS/TCP
exchange. A subsequent offline command requires every temporary ancestor to be
removed while preserving the symlink. A second enabled command renames an existing
parent, creates an author-owned replacement, and checks that the mounted resolver
remains read-only at the renamed location. The final offline command requires the
replacement content to survive and the temporary file in the old parent to be gone.

Run `resolver-replaced-scaffold.yaml` with `--network-fixture`, then verify using
`verify_resolver.py RUN_DIRECTORY --negative`. This command replaces a directory
created by resolver setup. Cleanup must reject the identity mismatch and refuse
publication. Expected build exit is 1. The verifier does not claim completed
cleanup or independently inspect the rejected writable layer. Rootless
ResolverTarget tests separately verify that replacements are not deleted.

These are benign modifications of the disposable command filesystem, with no
access to supervisor files or host namespace operations. They qualify these
specific lifecycle paths on the tested VM, not arbitrary concurrent rename
schedules or every filesystem/kernel/platform combination.
