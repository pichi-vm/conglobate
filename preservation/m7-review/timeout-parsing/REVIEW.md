# Clock-independent timeout parsing

Historical finding errors-api-007, implemented after the needs-work M4 review.

- Removed Instant::now().checked_add from CommandTimeout::from_str. Parsing still
  checks positive integer/unit syntax and checked multiplication to milliseconds.
- No arbitrary policy cap added. PreparedStep::start retains checked_add before
  admission and returns the owned Ready failure on deadline overflow.
- Added representation-boundary assertions: maximum milliseconds and maximum
  convertible seconds parse; the next second value fails with Overflow.
- Validation: cargo +stable test --test values --test recipes passed (11 tests);
  cargo +stable fmt --all -- --check and git diff --check passed. Cargo used the
  existing m7-stable target and /var/tmp scratch with two jobs. Logs preserved.
- This is a scalar-input change, not new sandbox/VM qualification. No resources
  or authority are acquired by parsing. The consuming execution failure path is
  unchanged. No complete milestone adherence pass is claimed.
