# Current documentation reconciliation

Reviewed at ca728ef with the README networking clarification. No application source changed. This is a scoped finding reconciliation, not a full milestone adherence pass.

- documentation-001: EnteredRoot::restrict no longer claims CARAPACE is unsupported. It executes the supplied script; src/step/directives.rs prefixes the command with export CARAPACE when a root is available.
- documentation-002: src/lib.rs describes the implemented pipeline and separates runtime qualification from implementation.
- documentation-003: Step documentation and ARCHITECTURE.md describe Ready/Running/Cancelling through Reaping, Draining, Quiescent and Finishing with retained failure ownership. Supervision documents identity exchange through admission. These match current consuming method signatures; obsolete LabelPolicy is explicitly identified as removed.
- documentation-004: Volume documents best-effort lazy detach on abandonment and explicitly denies that it proves flush completion or yields an Unmounted receipt.
- documentation-005: Pipeline::new/run, BuildRequest::parse_from, Inventory::decode, TransformSelection::apply, HelperInvocation::parse, StagedDirectory::create and Cache::open have summaries and applicable error contracts. Infallible Pipeline::new needs no error section.
- documentation-006: ChildReaper documents process-lifetime subreaping and absence of a destructor that disables it. Current implementation has no such Drop.
- documentation-007: PrivateMountNamespace documents destructor restoration and explicit error handling. NamespaceLease::drop attempts setns; commit consumes the restoration descriptor so subsequent Drop cannot restore.

Validation: strict `RUSTDOCFLAGS="-D warnings" cargo +stable doc --locked --no-deps` passed; `cargo +stable test --locked --doc` passed all 53 doctests. Both used CARGO_BUILD_JOBS=2, TMPDIR=/var/tmp/conglobate-rewrite-tmp and CARGO_TARGET_DIR=target/m7-stable. Raw logs are adjacent. Rustdoc alone does not establish prose correctness; the source comparisons above support the dispositions.

README now explicitly states environment ownership of networking and the two required inputs. The preceding namespace admission regression passed (one targeted test); no new VM execution is claimed.
