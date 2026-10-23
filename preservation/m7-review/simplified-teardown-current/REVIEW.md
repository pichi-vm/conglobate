# Post-command cleanup on simplified lifecycle

Four disposable Fedora 7.2.10 native amd64 cases pass at a4af392: held cleanup,
missing cleanup acknowledgment, cleanup timeout and delayed teardown. Each uses
the production execution/cleanup owners with trusted test helpers. Successful
shell exit cannot substitute for cleanup, missing completion is rejected,
timeout retains resources and delayed teardown cannot renew its cutoff. See
individual assessment.json and serial.txt for actual assertions and outcomes.

All selected source archive files are compared to a4af392 in source-comparison.
These are not arbitrary-scheduling proofs or full distro/CC qualification. The
blocked-descendant and writeback scenarios have not been rerun on this revision.
No complete adherence pass is awarded. Large guest assets remain local with
hashes rather than durable M8 archives.

Source audit also identifies init PID/mount coupling in anchor/namespace.rs as
remaining recovery-era machinery. The design requires an independent environment
controller, not that conglobate share PID 1's mount namespace. Any replacement
must still reject cleanup in a different mount namespace than acquisition and
verify attached control resources. No change to those checks is made here.
