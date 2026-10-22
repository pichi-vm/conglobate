Authenticated ancestry rejection matrix
=======================================
Actual non-root Chain::stage tests construct a valid three-layer chain with
identical COW bytes but different authenticated parent salts. Reversed OCI
ordering must reconstruct the expected roots/parents. Rejections cover a missing
middle ancestor, duplicate top descriptor, duplicate root via raw plus zstd
transport, disconnected genuine layers, zero top root with layers, and a nonzero
root without layers. Error categories are asserted, not merely non-success.

The whole scutes integration test target passes four non-root tests; two kernel
tests are explicitly ignored and are NOT counted as validated by this run.
They have separate VM evidence elsewhere. No new production code was needed.

Scope: authenticated graph reconstruction and existing sparse/extent tests.
This does not qualify malformed persistent snapshot exception records, kernel
metadata parsing, every decompressor failure, or generic recipe execution.
A cyclic ancestry cannot make traversal unbounded: Chain::stage removes a
candidate per step; a repeated root has no remaining candidate and errors.
No cryptographic fixed-point cycle was fabricated for this test.
