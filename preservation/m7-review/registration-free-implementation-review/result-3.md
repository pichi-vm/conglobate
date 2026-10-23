Confirmed independently. `await_admission_until(self)` takes the channel by value at `:46`, so `Supervision<Admitted>` binds to the closure-local `channel` at `:155`; a failure of `gate.start_until` at `:156` drops it — running `ControlSocket::drop`'s `shutdown(Both)` — before the recovery block at `:166` can drain diagnostics or clean up.

## One new finding: the fix is narrower than the hazard

**Medium — residual early-cancellation race at `src/isolation/receiver.rs:156`.** The `09d698c`/`24ccd81` fix hoisted `gate.wait_ready_until` out of the closure, which was correct and load-bearing. But the closure at `:153-158` still moves the channel, so the same hazard survives for its remaining failure points. The clearest case is `gate.start_until(setup_deadline)?` at `:156`: admission has *already succeeded*, so `Supervision<Admitted>` is bound to the closure-local `channel` at `:155` and drops the instant `start_until` returns `Err` — closing the socket before the recovery block runs `running.wait_until` and `diagnostics.finish()`.

I verified the mechanism rather than taking it on report: `await_admission_until(self, ...)` is by-value (`src/process/supervision/admission.rs:46`), and `ControlSocket::drop` (`src/process/supervision.rs:40-45`) issues `shutdown(Both)`. The consequence chain the reviewer traced is the same one the original fix was written to prevent — channel loss → supervisor sees `"joining peer closed"` → teardown → `cgroup.kill` → SIGKILL, with the 30s `TEARDOWN_TIMEOUT` bounding only the wait-for-empty, not the kill.

The two sibling calls (`report_prepared_until`, `await_admission_until`) matter less — the channel is typically already broken when they fail — and `await_admission_until` is by-value by design, so it is harder to restructure. The `start_until` case is the one where a healthy channel is demonstrably closed early. **Pre-existing, not a regression introduced by this range.**

## Assessment of the rest of that review

It **confirms** the fix is correct on every point I asked about, with mechanism-level evidence I spot-checked:

- **Channel lifetime:** `self.channel` is untouched in the new arm and outlives `mount.close()` at `:147`. The old code was genuinely broken — the closure captured `self.channel` by move *at construction*, regardless of which line failed at runtime.
- **Ordering:** `wait_until` before `diagnostics.finish()` is structurally safe, not merely conventional — the direct child unshares `CLONE_NEWPID` and its PID 1 is gone before the parent is reaped, so every pipe write end is closed and `finish()` cannot block.
- **Deadlines:** `setup_deadline` minted once at `:83`, reused; no clock read, no sleep in the new arm.

Two points I'd downgrade from how they were framed:

- The reviewer's **Finding C** (caller stringifies the error at `src/execution/helper.rs:396`, so `UnmountFailure::cleanup()` is never called) — I reported this myself as *intentional*, not a defect. The caller is an exit-only helper; retained owners there serve diagnostics. Calling the retry affordance "dead weight" overstates it: `Cleaned` vs `Cleanup` still distinguishes clean from failed teardown in the message, which is exactly what `label-admission-off-v2` relies on.
- The reviewer's **Finding D** (the new arm only reaches `mount.close()` on non-deadline failures) is a correct and useful observation, and the right conclusion is the one given: no change, since the alternative is deadline renewal, which the design forbids.

## Net effect on my report

Adds one Medium finding (`receiver.rs:156`). Everything else either confirms the fix or restates items I already reported. My earlier dispositions stand, including that five of my findings were addressed by `c1073b9..a3c528a` and that three Mediums remain open (`labels.rs` two-filesystem self-evidencing, probe error-context asymmetry, `bootstrap-unpinned-v1` not re-derivable).

Scope limits unchanged: amd64/KVM only, unmigrated harness still outstanding, no milestone gate claimed. No edits, no privileged operations, no proposal to restore policy registration.
