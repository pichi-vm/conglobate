# Milestone 5 adherence review

Status: initial review with a finding; clean streak **0/3**. M5 is not accepted.
Starting implementation: `1cd3b69`. This review covers changes since the M4
accepted implementation scope `ed0fecd408562aed8bc083e8fbe412c4b80ec514`.
The complete milestone boundary remains DESIGN.md, SANDBOX-DESIGN.txt,
ARCHITECTURE.md and REIMPLEMENTATION.md; evidence does not narrow that boundary.

## R1 — unfinished formatter inputs dropped without a reaping proof

`Formatting` held `Process<Running>` followed by an ordinary boxed `FormatInputs`.
`FormatWaitFailure` owns that same `Formatting`. Dropping either first invoked
`ChildLease::drop`, which explicitly performs only bounded best-effort termination
and may return without reaping. Normal field destruction then removed formatter
runtime backing and requested snapshot teardown. Kernel deferred removal protects
live device holders, but it is not the explicit quiescence proof required by the
owner's documented lifetime guarantee. The runtime and mapping owner must remain
retained if explicit reaping has not completed.

Correction: retain the input box with safe `ManuallyDrop`, as active command
storage already does. Release it only after successful `wait_until` or successful
`terminate_until`. Launch failure has no child and retains ordinary returnable
inputs; completion failure follows a successful reap and may return inputs.
Dropping a running formatter or a wait/cancel failure retains inputs for dedicated
VM disposal. No watchdog, synchronization sleep, unsafe code or new dependency.

Validation:

- v183: fixed-deadline timeout/cancel/reap and real mke2fs snapshot-exhaustion
  rejection still pass, including explicit runtime/backing cleanup.
- v184: drop the running formatter owner; its snapshot mapping remains present.
- v185: reach the actual setup deadline, then drop the wait failure; its snapshot
  mapping remains present. Both drop cases are terminal disposable-VM tests and
  do not attempt another build or recover the retained owners.
- v186: full normal VM suite passes, including successful formatting, bootstrap
  execution acceptance, metadata persistence and verified sealed output reopening.
  R1 is addressed; this does not complete the full milestone review.
- 56 library tests, 49 doctests, root and fixture strict Clippy pass.

## Initial source review coverage

Read the bootstrap geometry/formatter/dual-mount requirements and examined:

- formatter capture, runtime staging, typed ext4 profile/configuration, descriptor
  transfer, private helper root/environment, successful execution, failure and Drop;
- source-level ownership of snapshot backing and deferred mapping destruction;
- paired detached-view namespace identity checks, confined `/mnt` attachment,
  and ownership of the root/output namespace handles through attachment;
- direct sealing's use of devmap-snapshot wire codecs, extent boundaries, retained
  recorded zero writes, single data hash stream and immutable reader authority;
- the distinction between formatter success, command acceptance, clean storage
  finalization, sealing and future publication/inventory.

The R1 discovery makes this a non-clean review. Full acquisition/transfer/root
attachment/process teardown, pair finalization, generic API/typestate/style and
requirements-to-evidence review remain before starting a clean streak. Repeated
VM runs are regression evidence, not adherence passes. M6 publication/inventory,
M7 distro/architecture/confidential-platform qualification and M8 retirement are
not inferred from these tests. Legacy code and exploratory evidence remain intact.
