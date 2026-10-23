# Cgroup joining terminology

Production containment state Registered is now Joined; DomainRegistration is
DomainJoin and the exchange is join_helper_until. The cgroup2 descriptor check,
self-placement, acknowledgment, descriptor closure, cancellation and retained
failure ownership are unchanged. Packet wire opcodes are unchanged. The source
module names now describe joining. Historical captured sources remain untouched.

Validation: cargo +stable test --lib --tests passed (115 unit tests plus the
integration targets); 54 doctests passed; strict all-target Clippy passed.
The maintained command-integration fixture compile is checked separately.
The old rewrite-policy-loader fixture still references obsolete guardian APIs
from earlier history; it is preserved as precedent, not a current passing test.

This is one migration step, not removal of the policy implementation and not
an M4 adherence pass. Policy ownership and BPF dependencies remain to remove.
