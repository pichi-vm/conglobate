# Command-status gate transport: first M4-T1 implementation step

ExecutionGate now has a sealed Started state reached by the consuming
start_reporting_until transition. Its only added observation is a terminal
shell ExitStatus. The paired CompletionEndpoint is obtained after readiness
and explicit start; report consumes it and performs one nonblocking packet
send (retrying EINTR only). Neither type exposes a raw descriptor, cancellation
operation, filesystem authority or successful-storage receipt.

The versioned fixed packet carries only a terminal wait status. Wrong shape,
version, nonterminal status, truncation and ancillary rights fail. CLOEXEC
comes from the original socketpair. Drop retains the parent's shutdown behavior.
There is no renewed relative timeout: observation takes an existing cutoff;
report cannot wait on teardown. Nonblocking backpressure is an explicit error.

Four added regressions cover observation while reporter cleanup is held by an
explicit channel gate, malformed/missing status, expired observation, and rejected
SCM_RIGHTS with proof the received descriptor closes. Ordinary/nonzero/signaled
statuses survive. Existing readiness/start tests and full rootless suite pass;
strict Clippy passes. No sleeps or privileged host operations.

This is an intermediate, tested transport change. The production receiver still
uses the original gate methods and reports completion after local cleanup.
M4-T1 remains OPEN. Do not claim correct execution/teardown timing or init FD
isolation from these tests. Next connect the resident init's consumed reporter,
add a narrowly owned descriptor audit allowance (never image-child access),
and separate supervisor shell observation from mandatory receiver cleanup
acknowledgment under one fixed teardown cutoff. Remove obsolete alternate gate
entry paths when their actual callers migrate. Full composed VM regressions
listed in command-timeout-boundary/FINDING.md remain required.
