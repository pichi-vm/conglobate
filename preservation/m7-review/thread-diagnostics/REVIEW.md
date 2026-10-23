# Shared thread diagnostics

Baseline: 9e729d1. Finding: simplification-002.

Ten sites duplicated fallible enumeration of /proc/self/task. They now capture
an immutable crate-private ThreadSnapshot and inspect is_single(). No public
API or authority token was added. The snapshot explicitly cannot establish
continued exclusivity; trusted helper startup remains responsible for avoiding
concurrent thread creation. No sleeps or retry loops were added.

Source review: all ten call sites retain their contextual diagnostic messages.
ChildReaper retains its PID-first short circuit; domain joining retains its
thread-first check. PrivateMountNamespace still maps both directory opening and
entry iteration errors to NamespaceFailure::Before. The remaining callers still
propagate io errors. Enumeration uses try_fold, reads every successful entry,
and returns on the first error exactly as the previous fallible collect did.
It avoids allocating the discarded Vec of entries. No namespace transition,
capability/seccomp policy, cleanup owner or admission protocol changed.

Validation (serial, CARGO_BUILD_JOBS=2,
TMPDIR=/var/tmp/conglobate-rewrite-tmp, CARGO_TARGET_DIR=target/m7-stable):

- cargo +stable test --lib: 131 passed.
- cargo +stable clippy --all-targets -- -D warnings: passed.
- cargo +stable check --manifest-path tests/lifecycle/Cargo.toml: passed.
- git diff --check -- src: passed.

No new privileged VM run; existing tests and compilation do not prove freedom
from thread races. This is a focused equivalence/style review, not a complete
adherence pass. M4 remains 0/3. Network topology qualification remains open.
