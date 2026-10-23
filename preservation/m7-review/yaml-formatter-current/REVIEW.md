# YAML budget and formatter ordering regressions

Base: 1bda74f. No production behavior changes.

- tests-008: added a flat 101-alias scalar input with one anchor, plus a successful one-alias control. Possible output remains tiny even if enforcement regresses. The actual wrapper rejects with an alias-ratio diagnostic. Previous depth test remains. Exact serde-saphyr 2.0.0 defaults set minimum aliases 100 and ratio multiplier 10, depth 64 and total scalar bytes 64 MiB. Scalar-budget boundary execution and recursive expansion behavior are not claimed; this finding remains partially addressed.
- correctness-001: current Formatter::try_from(FormatReceipt) explicitly sorts wire RuntimeFile entries by String path. Formatter::validate compares the same String key. Added a validator regression that accepts usr/lib-x before usr/lib/a and rejects Path-component ordering. Prior producer execution evidence (v259 failure/v260 success) remains historical; this new rootless test does not execute a formatter or forge a FormatReceipt.

All 150 library tests and strict all-target Clippy pass. Commands use Cargo +stable with --locked, CARGO_BUILD_JOBS=2, TMPDIR=/var/tmp/conglobate-rewrite-tmp and CARGO_TARGET_DIR=target/m7-stable. Logs and exact source hashes are adjacent. No VM run or full milestone pass is claimed.
