# YAML scalar budget boundary

Base: 3d19197. Finding tests-008 now has maintained depth, flat alias-ratio and scalar-boundary regression coverage through the production YamlDocument::decode entrypoint.

The added test constructs one flat ASCII scalar at exactly 64 MiB, verifies successful decoding and exact contents, drops that decoded copy, then appends one byte and requires the typed ScalarBytes error with count 67108865. No custom budget, aliases, recursion, unchecked expansion or privileged operations are used. Even if enforcement regresses, input size remains fixed. The test verifies this parser limit, not a bound on all parser allocation/CPU or general resistance to resource exhaustion.

Inspected locked serde-saphyr 2.0.0 budget defaults and enforcement: cumulative scalar byte counts breach only when greater than max_total_scalar_bytes. Error::WithSnippet wraps the typed Budget error. The initial test used the wrong public enum module and failed to compile; the next assertion incorrectly expected an unwrapped error and failed despite correct rejection. Both logs are retained. The corrected test explicitly unwraps snippet context and checks the real typed cause. All three wrapper tests pass in 3.06 seconds, strict all-target Clippy passes. Initial formatting check required wrapping one expression; final formatting passes.

Only cfg(test) code changes. Production parsing, limits, public API and ownership are unchanged. No production defect was found; no milestone pass is awarded by this test. M4–M6 and broader qualification remain open. This closes the previously identified scalar coverage gap without claiming recursive expansion testing or changing environment resource policy.
