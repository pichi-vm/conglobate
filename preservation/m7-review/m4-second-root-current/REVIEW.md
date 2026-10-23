# Current two-root label admission failure

Native amd64 disposable KVM run at production baseline 0345372. The environment
fault fixture reports exactly one allowed probe on the tool filesystem and one
denied probe on the bootstrap output filesystem. The build rejects before the
first image command, reports completed step cleanup with no runtime-retention
message, and leaves no output directory. Runner exit 1 is expected; the independent
negative verifier passes all nine checks. No unexpected failure is counted green.

This refresh exercises current shared loop ownership, streamed materialization,
external network inputs (unused by the offline recipe) and typed stage rejection.
It also exercises successful trusted policy-fixture compilation after diagnostic
wrapping. Source comparison records exact application/fixture correspondence.
Cleanup evidence is CLI diagnostics, not independent kernel-resource enumeration.
The raw status, serial output, source/context, fixture sources and build logs are
preserved. Large assets/loaders are hashed locally, not remotely archived.

No full M4 adherence pass or confidential-platform qualification is claimed.
