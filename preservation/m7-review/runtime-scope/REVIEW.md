# Shared explicit runtime lifecycle

Implements accepted simplification-011 after ba58d4c.

Runtime::scoped owns create, the trusted stage callback and explicit close. It
runs close on both returned stage outcomes. Successful output is returned only
when close succeeds; otherwise output is dropped. Panics/crashes remain covered
by the dedicated-VM disposal contract, not a new guaranteed cleanup promise.

RuntimeScopeFailure owns the creation/cleanup failure and preserves the stage
error in the combined case. Its lifetime retains the workspace dependency.
The callback cannot return a borrow of the runtime being consumed by close,
checked by a new compile-fail doctest. The pipeline conversion preserves an
original typed BuildError when cleanup succeeds. Terminal cleanup failures
still render at the pipeline boundary as before; this does not resolve the
broader end-to-end typed diagnostic finding.

Bootstrap, carapace and firmware all use the shared method. Bootstrap formatter
staging and its own failure cleanup stay inside the callback, so every returned
staging error still reaches runtime.close. No command admission, close deadline,
resource destruction order inside Runtime::close, or publication gate changed.

Validation: 126 library tests and 53 doctests passed; strict library Clippy passed;
formatting and git diff --check passed. The first attempted combined --lib --doc
command was rejected by Cargo; its log is retained and each target was then run
separately. No VM run occurred. A current integrated runtime/CLI run remains part
of the open M4 qualification; source equivalence is not claimed for older runs.
This focused review is not one of the three complete milestone adherence passes.
