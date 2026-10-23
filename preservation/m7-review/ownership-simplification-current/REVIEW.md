# Ownership and simplification reconciliation

Base: 6239c71. Only production change is borrowed plan-deduplication keys and consuming the temporary map before moving the recipe.

- ownership-003 (revalidated rejection): Inventory decode still constructs FilesystemLabel from a cloned String. This cold-path clone preserves a single validated-value constructor; no measured impact justifies a second validation API.
- ownership-004 (revalidated rejection): Assembly borrows a temporary Vec of scute references; bootstrap needs emptiness, first parent, length and iteration. An iterator or generalized slice interface is feasible but not a necessary correctness fix or demonstrated performance improvement. Retain the simple private slice contract.
- ownership-005 (fixed after reconsideration): Borrowed ImageReference map keys are feasible: consume inputs into the final Vec before moving recipe into BuildPlan. Implemented that ordering and removed the extra key clones. The previous claim that the move inherently prevents borrowing was too strong.
- ownership-006 (superseded by external networking): src/network/nat.rs and NatRules are gone. The cited NAT-table String clone no longer exists.
- ownership-009 (revalidated rejection): Transform compares final owned tree equality with the original and returns exact input bytes when operations cancel. Per-operation changed flags do not preserve this requirement, and encoding first may normalize bytes. Retain comparison copy.
- simplification-014 (revalidated partial simplification; retained alias intentional): RolledBackPreparation and BootstrapExecutionFailure are absent. ExecutedBootstrap remains as the concrete two-volume completion alias with a compile-fail non-forgeability example. The blanket removal was not the accepted subset; do not label all three removed.
- simplification-015 (revalidated rejection): into_volume/into_rejected_volume remain typed convenience conversions with four current preserved integration-harness callsites. Remove only with deliberate caller migration/retirement, not as supposedly unused APIs.
- simplification-016 (revalidated fixed): RunningCommand::wait_for_exit is absent from current setup source.
- simplification-020 (revalidated fixed): COPY mode exports only CopyMode and ModeError. Change, Clause, Operator, Permission and Permissions are private implementation types.
- simplification-021 (revalidated rejection): Public sealed CheckoutState exposes StagedResources and ReadOnlyResources as associated type values; fields remain restricted. Demoting these types violates the public interface, regardless of explicit consumer spelling.
- simplification-024 (revalidated fixed): EnteredRoot provides restrict; the execute method belongs to RestrictedCommand. The unused EnteredRoot convenience wrapper is absent.
- simplification-025 (superseded by external networking): VethPair and its module are absent; external environment owns links.
- simplification-026 (revalidated rejection): FormatterCleanupFailure::source_error has a current unit-test caller asserting NotADirectory. Both formatter error owners retain the common borrowed error-inspection API alongside consuming recovery. A claim of zero use is no longer correct.
- simplification-029 (revalidated fixed): Packet::receive forwards directly to receive_shapes; the intermediate receive_from is absent. The negative test also calls receive_shapes directly.

All 229 all-target tests and strict all-target Clippy passed with Cargo +stable --locked, CARGO_BUILD_JOBS=2, TMPDIR=/var/tmp/conglobate-rewrite-tmp, CARGO_TARGET_DIR=target/m7-stable. Raw logs adjacent. No new privileged execution. M3 is reopened at zero because its planning implementation changed; prior passes remain historical. No scoped review is counted as a complete milestone pass.
