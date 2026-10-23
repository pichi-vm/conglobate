# Remove persisted command-domain recovery records

User correction: implementation should be simple and straightforward. Build
commands remain untrusted; disposal of the dedicated VM handles supervisor death.

Removed cgroup CBOR serialization, record publication/load/removal, record identity
reopening, RecordedDomain, record-cleanup failure owners and the Step Finishing
phase. CommandDomain owns the live cgroup descriptor and lends a joining permit;
consuming cancellation cannot return join authority. Setup rollback now owns at
most an empty cgroup target, without branches for partial record publication.
Helper termination, cgroup cancellation, reaping, adopted-child draining, fixed
teardown deadlines and storage finalization ordering remain.

The removed code, parser tests and obsolete publication fixture are copied under
preservation/pre-domain-record-removal, as well as recoverable through Git. Those
record-specific tests no longer describe a supported protocol. They are not
counted as containment coverage. The maintained contained case is updated to use
CommandDomain directly and assert that no recovery record appears.

Validation: full rootless test suite and doctests pass (see tests.log); strict
all-target Clippy passes. Disposable Fedora 7.2.10 guest proves launch failure,
join failure, pre-admission descendant cancellation and abrupt helper death,
with clean exit/poweroff. Exact fixture source and logs are retained. The only
subsequent production edit changes the anchor module description, not behavior.
Large guest assets remain local with hashes, not durable M8 archives. No claim
of complete sandbox, distro or confidential-platform qualification is made.

Architecture M2 reopened 0/3; M4–M6 remain 0/3. Remaining simplification includes
runtime directory/init coupling, attached workspace ownership and teardown
orchestration. The environment-prepared context interface is recorded in the
flow sketch, not implemented here. No complete adherence pass is awarded.
