# Binary simplification evidence

See ../../SIMPLIFICATION.md for scope, review gates and limitations. These are
selected reports; full per-run source snapshots, inputs, serial logs and status
images remain in /storage/conglobate-validation/simplification on the validation
host, including failed/preliminary attempts. No confidential hardware or new
distro-boot claim is made. Hosted native CI receipts are added after completion.

The size audit applies the same extraction to 8c2d294 and the simplified source:
all src/*.rs recursively, excluding fixture.rs, tests.rs and firmware/tests.rs;
remove top-level #[cfg(test)] mod tests blocks, retaining other declarations.
Run tokei --output json on each extracted tree. Count physical lines separately.
Test-only methods outside those modules remain counted. The full measure.py and
extracted trees are retained alongside the local results. This corrects the early
baseline estimate; it does not count moving code to tests as production savings.
