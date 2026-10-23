# External-policy production admission control

The new binary has no policy loader/build dependency. The trusted VM loads the
fixed hook. Restricted anonymous-inode validation succeeds, the command emits
COMMAND_LABEL_ADMISSION_PASS, and conglobate finalizes/publishes the artifact.
The runner validates exact export success, the independent exit-0 result-device
record, and all exported blob hashes. No boot of this minimal derivative was
attempted. This is native amd64/KVM single-root coverage, not full sandbox,
bootstrap, Arm or confidential-platform qualification.

The matching off-v2 run rejects the command with the expected label EPERM and
clean cleanup diagnostics. Both v1 failures remain preserved separately.
