Root-required COPY regression tests in a disposable guest
=========================================================

Reproduce:
  python3 validation/sandbox/guest.py copy-tests \
    --kernel-version 7.2.10-200.fc44.x86_64 --out /tmp/NEW-COPY-ROOT-TESTS --run

The harness builds the actual tests/copy.rs integration test executable using
Cargo.lock, packages its dynamic dependencies and records the test source plus
production sources. Inside the disposable guest it runs all five tests with
--include-ignored on tmpfs, as initial-namespace root. The timeout is 30 seconds.
The verifier requires five passes, zero failures and zero ignored tests.
No host disks, shares or NICs are exposed; host SELinux is unchanged.

All five pass: COPY contents/patterns/link/ownership behavior, explicit access
ACL and revision-2 capability xattr preservation, appended-symlink replacement
and existing-directory metadata behavior, absolute image-link confinement/cycle
rejection, and the new regular-file/.. path regression. This closes the skipped
root checks from the preceding local path-fix run.

Limits: initial-root kernel tmpfs, not mapped FUSE. Guest SELinux is disabled;
the security.selinux assertion compares the absent attribute in this fixture,
so this is NOT an enforcing-SELinux or foreign-label test. Other dedicated
fixtures supply that evidence. It is not a full trusted-COPY executor test or
qualification for concurrent image-path mutation.

Evidence: reproduced/copy-root-tests (test build command/log, guest command,
kernel/runtime hashes, test/production source snapshots and result).

Trailing-directory follow-up: the current harness requires six tests, all of
which pass in reproduced/image-path-trailing-directory. The added test preserves
kernel directory requirements for trailing slash/dot in direct paths and symlink
targets; older five-test evidence remains scoped to its captured sources.
