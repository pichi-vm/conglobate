# Separate shell completion and receiver cleanup

Namespace init reports terminal shell status before exiting. The exact owned
status descriptor is the only additional init FD allowed by the audit; CLOEXEC
is checked before spawn, init remains non-dumpable and actual command FD/proc
checks pass. Standalone legacy execute has no reporter. Reporter absence is an
optional authority for that separate API, not an acceptance flag.

The contained receiver waits for status under supervisor control, forwards it,
then waits/reaps its namespace helper and drains diagnostics. It rejects a
helper exit inconsistent with the reported shell outcome. It then restores
resolver state, closes the view and restores its namespace before sending the
new Cleaned message. This message cannot substitute for primary shell status.

CommandResult::teardown chooses AwaitingCleanup only for observed shell exit;
observation failure goes directly to cancellation. AwaitingCleanup owns the
running Step and the one fixed teardown cutoff. Its failure retains the running
phase for cancellation retry, and the higher execution failure owner cannot
promote cleanup recovery to accepted storage. Expired deadlines still require
VM disposal if cleanup cannot complete. No timeout is renewed by retry.

Protocol tests explicitly gate cleanup after primary completion, reject status
in place of cleanup, and reject peer loss. Rootless suite/doctests and strict
Clippy pass. Initial compile failure used the wrong rustix module for getfd;
its diagnostics remain preserved. Actual confinement CLI build/export and busy
finalization rejection VM tests pass. Captured-vs-current source differences in
the first VM run are comments and added unit tests, recorded exactly in its
post-capture.patch; second run matches all production Rust bytes.

M4-T1 correction is implemented, not fully qualified. Next add composed delayed
cleanup/descendant, missing-cleanup acknowledgment/helper-death, and timeout
regressions; rerun network resolver failure paths. No clean adherence pass or
portable/confidential qualification is claimed. Keep all existing evidence.
