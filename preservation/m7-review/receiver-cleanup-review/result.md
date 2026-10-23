I have everything needed. Findings below.

## Assessment of the receiver refactor

**Consuming ownership — correct.** `ReceiverStartup` takes `mount: Mount<'ns, InUse>`, `diagnostics: DiagnosticRelay`, `deadline: Instant` by value, and both exits consume `self`: `failure(self, source)` and `finish(self, channel)`. Since `failure` is the only way to reach cleanup and it moves the struct, no path can drop the mount or relay without having gone through reap→drain→close. That is a stronger structural guarantee than the previous inline arms, which duplicated the sequence twice and could drift.

**Channel lifetime — my finding D is resolved, and the three arms are correctly differentiated.** I checked the receivers:

- `report_prepared_until(&self, ...)` (`admission.rs:17`) — borrows. On failure, `self.channel` is still a live local in `execute`, so `startup.failure()` runs reap/drain/close with the channel open.
- `await_admission_until(self, ...)` (`admission.rs:46`) — by value. On failure the channel is consumed and dropped by the callee, so `ControlSocket::drop`'s `shutdown(Both)` fires before cleanup. **This arm is unchanged and cannot be fixed without changing that signature.** It is also the benign case: the channel is already broken (EOF/malformed/timeout) when it fails.
- `start_until` (`gate.rs:80`) — the one that mattered. `channel` is bound in the *outer* scope at `receiver.rs:149` and is still owned when `startup.failure(source)` runs at `:159`. The comment `// `channel` stays alive while failure() reaps/drains/closes the view.` matches the code. **Finding D (receiver half) closed.**

**Gate drop on the two early paths — correct and necessary.** `drop(gate)` before `startup.failure(...)` on the `report_prepared`/`await_admission` arms closes the parent socketpair end. The leaf is blocked in `ExecutionEndpoint::ready_and_wait` → `self.0.receive(2, deadline)` (`gate.rs:98`), whose `Socket::wait` returns `BrokenPipe "execution gate peer lost"` on peer closure (`gate.rs:117-121`). Without this, the leaf would sit until its own deadline and `mount.wait_until` would block for the full remaining budget. This is a readiness-primitive release, not a sleep — consistent with the no-sleep requirement.

**Deadline behavior — clean.** `deadline: setup_deadline` is captured once into `ReceiverStartup` and reused in `failure()` via `self.deadline`; no clock read, no renewal. The split in the doc comment is accurate: failure waits under the original setup deadline, `finish()` uses `wait_under_supervisor()` (no deadline argument) under the supervisor's independent recipe deadline.

**Recovery — preserved.** `failure()` keeps the established order: `wait_until` first (so pipe writers are gone before joining), then `diagnostics.finish()`, then `mount.close()` yielding `Cleaned`/`Cleanup`. On the `wait_until` error sub-path it returns `ReceiverFailure::Startup{mount: failure.into_parts().1}`, retaining the owner — the relay is dropped unjoined there, which is the pre-existing exit-only-helper behavior we already agreed to keep.

## labels.rs diagnostics — satisfies D

Context is now attached at every failure point: `create anonymous label probe`, `... while writing security.selinux`, `read environment label`, `remove environment label`, plus `open output for labeling`, and each is suffixed with `(command root /)` or `(command root /mnt)`. The `and_then(LabelProbe::verify)` composition is what lets one `map_err` cover both construction and verification without changing behavior. The readback-mismatch message is unchanged and still gets the root suffix.

Evidence confirms it end to end: `/var/tmp/conglobate-receiver-root-negative-v1/serial.txt:44` reads `environment labeling support: Operation not permitted (os error 1) while writing security.selinux (command root /mnt)`, with `allowed=1, denied=1` and all nine negative checks true. The root identification is real, not just formatted.

## On covering the start_until ordering with an existing-interface test

Yes — I think a small test is feasible without production fault flags, but it must be scoped honestly to the *lifetime ordering*, not the gate-send failure.

The tractable target is `ExecutionGate`/`ExecutionEndpoint` in isolation, via the already-public `ExecutionGate::pair()`. A rootless test can: create a pair, drive `wait_ready_until` to `Ready` by sending opcode 1 from the endpoint, then drop the endpoint and assert `start_until` returns `BrokenPipe "execution gate peer lost"`. That gives deterministic coverage of the error `start_until` actually produces, with no fault injection.

What it does **not** give is the receiver-level ordering — that `channel` outlives `startup.failure()`. Covering that needs a `CommandReceiver` with a real launched child and a real `Supervision<Joined>`, which is the VM harness's job. A rootless test could at best assert a *proxy*: pair a `Supervision`, confirm the socket is still readable from the peer after a simulated failure path. That risks mirroring the implementation rather than asserting a requirement, which you've consistently rejected.

My recommendation: add the `ExecutionGate` `BrokenPipe` unit test (cheap, real, no flags), and record the receiver-level ordering as covered by inspection plus the existing negative VM runs — explicitly **not** as deterministic post-admission gate-send coverage. I am not inferring that coverage from the readiness failure, per your instruction.

## Ledger update

**Closed (2)**
- Probe diagnostic context (D, labels half) — implemented and demonstrated by `receiver-root-negative-v1`.
- `gate.start_until` channel-lifetime race (D, receiver half) — `channel` now bound in outer scope and alive through `startup.failure()`.

**Open (3)**
- Deterministic post-admission gate-send failure injection — not covered; the readiness path does not substitute. Mitigation proposed above.
- `await_admission_until` consumes the channel on failure — benign (channel already broken) and not fixable without a signature change; recorded as accepted residue, not a defect.
- Durable artifact archiving (M8).

**New findings: none.** The refactor introduces no new defect I can identify. The duplicated-cleanup risk present in the two prior inline arms is reduced, not increased.

**Scope limits unchanged:** native amd64 ordinary VM only; positive bootstrap regression still running, so I claim nothing from it; 193 rootless tests / 50 doctests / strict Clippy are your results, not executions I performed. No full confinement or platform qualification inferred. No milestone adherence implied; acceptance remains yours. No edits, no privileged operations, no agents, and no restoration of registration.