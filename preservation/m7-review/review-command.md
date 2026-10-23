---
description: Parallel Rust code review. Maps context, computes diff coverage, dispatches 13 parallel review agents, evaluates findings with batched critics, synthesizes by severity. Findings only — bias toward simplicity.
argument-hint: "[branch | staged | unstaged | all | <path> | <commit-sha> | pr <N>]"
allowed-tools: Task, Bash(git:*), Bash(cargo:*), Bash(gh:*), Read, Grep, Glob
---

# /review — Parallel Rust Code Review

Goal: high-signal, low-noise review of Rust code. Findings only — no proposed patches. Bias hard toward simplicity.

---

## Severity rubric (used by every agent and every critic)

- **Critical**: undefined behavior, RCE, data loss, secret leakage, panic on attacker-controlled input. Merge blocker.
- **High**: correctness bug, broken safety invariant, breaking public API, security regression, missing tests for non-trivial logic.
- **Medium**: performance issue, API ergonomics, quality concern, doc gap on public API.
- **Low**: nit, style, minor doc gap on internal item.

Pass this rubric verbatim into every Step 3 agent and every Step 5 critic.

---

## Step 1 — Resolve scope

Resolve `$ARGUMENTS` to a concrete file set in this priority order:

| Argument | Meaning | Source command |
|---|---|---|
| `branch` | Diff vs merge-base of `main`/`master` | `git diff $(git merge-base HEAD main)..HEAD` |
| `staged` | Staged changes only | `git diff --staged` |
| `unstaged` | Working-tree changes only | `git diff` |
| `all` | Entire workspace | `git ls-files '*.rs'` |
| `<commit-sha>` | A single commit | `git show <sha>` |
| `pr <N>` or `<PR-url>` | A pull request | `gh pr diff <N>` |
| `<path>` | A file or directory | files under that path, recursive |

Default with no argument: `branch` if on a feature branch; else `staged` if anything is staged; else last commit (`git show HEAD`).

Print a one-line scope summary: `<N files, ±LOC, crate(s)>`. Stop and ask only if scope is genuinely ambiguous.

Record the diff-touched line ranges per file — Step 2's coverage task needs them.

---

## Step 2 — Context Mapper + Coverage (parallel, runs first)

Dispatch TWO Task subagents in a single message. Both feed Step 3.

### Sub-task 2A — Context Mapper (`subagent_type: general-purpose`)

```
You are mapping context for a code review. Read:
- Cargo.toml (workspace and per-crate)
- src/lib.rs or src/main.rs of touched crate(s)
- The diff itself

Return a TIGHT summary, ≤300 words, using these headings exactly:

## Crate type
One of: bin | lib | proc-macro | no_std lib | FFI shim | mixed. Note any `#![no_std]`, `#[cfg(feature = ...)]` gates that change shape.

## Primary operations (3–5)
The core functions/types this code exists to provide. Name each with file:line and a 1-line purpose.

## Trust boundaries
Does the diff touch: unsafe, FFI, deserialization of external input, network I/O, privileged ops (attestation, kernel, hypervisor), filesystem, IPC? List which.

## Hot paths vs cold paths
Which changed functions are likely hot (per-request, per-block, inner loop) vs cold (init, config, error formatting)?

No prose padding. No conclusions.
```

### Sub-task 2B — Diff Coverage (`subagent_type: general-purpose`)

```
Compute diff coverage for the touched crate(s).

PROCEDURE:
1. Detect tool: prefer `cargo llvm-cov` (requires `llvm-tools-preview` component); fall back to `cargo tarpaulin` on Linux x86_64; else report unavailable.
2. Run with JSON output (`cargo llvm-cov --json --all-features` or tarpaulin equivalent), building tests with the same features the Mechanical agent will use.
3. Filter results to lines present in the diff (use the per-file line ranges from Step 1).
4. For each diff-touched function, compute: total executable lines, lines covered, percentage, and approximate cyclomatic complexity (count of `if` / `match` arm / `while` / `for` / `?` / `&&` / `||` in the function body).

OUTPUT JSON ONLY:
{
  "tool": "llvm-cov" | "tarpaulin" | "unavailable",
  "reason": "<if unavailable, why>",
  "functions": [
    {
      "file": "<path>",
      "function": "<name>",
      "line_start": <int>,
      "line_end": <int>,
      "lines_covered": <int>,
      "lines_total": <int>,
      "coverage_pct": <float>,
      "complexity": <int>
    }
  ]
}

Include only functions with at least one diff-touched line. If tests do not compile, return `unavailable` with reason `"tests do not compile"`. Do NOT block on flaky tests — record them as `unavailable` with reason and move on.
```

Capture both outputs. Pass the Context Map verbatim into every Step 3 agent. Pass the coverage JSON into Agent H specifically.

---

## Step 3 — Parallel review (single message, 13 Task calls)

Dispatch all 13 simultaneously in one assistant message. Each agent receives:
1. The diff (or file list for `all`)
2. The Context Mapper summary
3. The severity rubric (verbatim)
4. Its own role-specific prompt below
5. (Agent H only) the coverage JSON from Step 2B

Use `subagent_type: general-purpose` unless a project-local agent of the matching name exists in `.claude/agents/`.

### Output format (every agent EXCEPT A)

Every agent returns JSON only, no prose wrapper:

```json
{
  "agent": "<your name>",
  "clean": true | false,
  "not_checked": ["<dimension skipped and why>", ...],
  "findings": [
    {
      "id": "<agent>-<NNN>",
      "file": "<path>",
      "line": <int>,
      "line_end": <int or null>,
      "severity": "critical" | "high" | "medium" | "low",
      "category": "<short tag>",
      "title": "<≤80 chars>",
      "claim": "<one sentence: what is wrong>",
      "evidence_quote": "<verbatim code snippet that proves the claim>",
      "suggested_fix": "<one sentence>"
    }
  ]
}
```

The `not_checked` field is required. Declare what you explicitly skipped (e.g., `"miri runnability — no nightly toolchain available"`). Synthesis surfaces this as negative-space.

Agent A is the only exception — it returns raw tool output in its own shape (see below).

### Agent A — Mechanical

```
Run, in this order, on the touched crate(s):
1. cargo fmt --all -- --check
2. cargo clippy --all-targets --all-features -- -D warnings
3. cargo check --all-targets --all-features
4. cargo test --no-run            (compile tests; do NOT execute them)

Return JSON only:

{
  "agent": "mechanical",
  "tools": [
    {
      "name": "fmt" | "clippy" | "check" | "test-build",
      "exit_code": <int>,
      "warnings": [
        {"file": "<path>", "line": <int>, "code": "<e.g. clippy::needless_clone>", "message": "<one line>"}
      ]
    }
  ]
}

No interpretation, no severity, no fixes. Raw signal only.

Note: cargo audit is OWNED BY Agent L. Do not run it here.
```

### Agent B — Ownership & Allocation

```
Review for Rust ownership and allocation issues.

CHECK:
- Every `.clone()` — would `&T`, `Cow<T>`, `Arc<T>`, or restructuring eliminate it? Hot-path clones are at least High.
- `String` parameter where `&str` or `impl AsRef<str>` works
- `Vec<T>` parameter where `&[T]` works
- `&String` or `&Vec<T>` (deref coercion target lost)
- `.collect::<Vec<_>>()` immediately re-iterated
- `Vec::new()` / `HashMap::new()` in a loop with no `with_capacity`
- Pre-known-size collections without `with_capacity`
- Boxed trait objects where a generic parameter would monomorphize
- `Rc` / `Arc<Mutex<_>>` where a single owner suffices
- `Arc::clone` in tight loops where a `&Arc` reference suffices
- Lifetime annotations that exist but are unnecessary OR fight the borrow checker (clones added to escape lifetimes)
- For no_std crates per Context Map: any std-only allocation pattern that slipped in

SKIP: clones in tests, clones of `Copy` types, clones of small `Arc`s outside hot paths.

Severity per rubric. Hot-path is per Context Map.
```

### Agent C — Unsafe & FFI

```
Review every `unsafe` block, `unsafe fn`, `unsafe impl`, and FFI call in the diff.

PER UNSAFE SITE:
- Is there a `// SAFETY:` comment naming the invariants required?
- Are those invariants actually upheld at every call site? (audit callers)
- Could this be safe? If yes, that's at least High.

PER FFI SITE:
- Pointer validity (non-null, aligned, in-bounds)
- Lifetime: who owns the buffer, who frees it
- Null-termination / UTF-8 validity assumptions
- Repr: `#[repr(C)]` on structs crossing FFI; explicit padding
- ABI: `extern "C"` matches the C declaration
- Panics across FFI boundary (must be caught with `catch_unwind` if `extern "C"`)

SPECIAL SCRUTINY:
- `mem::transmute`, `slice::from_raw_parts`, `*_unchecked`
- `MaybeUninit` read before fully initialized
- `Send`/`Sync` impls — is interior state actually thread-safe?
- Casts that change pointer provenance

Default severity: High. Probable UB: Critical.
```

### Agent D — Concurrency & Async

```
Review for concurrency and async correctness.

CHECK:
- `.await` while holding `std::sync::MutexGuard` — must drop before await OR switch to `tokio::sync::Mutex`
- `std::sync::Mutex` in async paths where contention will block the executor
- Blocking calls inside async fn without `spawn_blocking`: file I/O, sync HTTP, `std::thread::sleep`, CPU-heavy loops, `Mutex::lock` on a contended lock
- Missing `Send` bounds preventing `tokio::spawn`
- `tokio::spawn`ed tasks whose `JoinHandle` is dropped silently (panics swallowed)
- Channel usage: unbounded channels with no backpressure; bounded channels with no error path on `send`
- `tokio::select!` branches that aren't cancellation-safe
- `Arc<Mutex>` where `Arc<RwLock>`, message passing, or sharding would be cleaner
- Async cancellation: drop-safety of in-flight state
- `loop { select! { ... } }` without a shutdown branch
```

### Agent E — Errors & API Design

```
Review against the Rust API Guidelines (https://rust-lang.github.io/api-guidelines/). Forward-looking API quality — Agent M handles semver-diff.

ERRORS:
- Every `unwrap()`/`expect()` outside tests — each needs justification or becomes a finding
- `panic!`/`unreachable!` in library code where Result is appropriate
- Errors that lose context (no `#[from]`, no `.with_context()`, no source chain)
- `anyhow::Error` exposed in a library's PUBLIC API (use a typed error there; reserve anyhow for bins)
- `?` that erases a variant the caller needs to discriminate
- Error types that aren't `Send + Sync + 'static`

PUBLIC API:
- Public types missing `Debug`, OR sensitive types accidentally `Debug` (secrets, keys, attestation material)
- Public types missing `Send`/`Sync` they could have
- `&self` vs `&mut self` vs `self` correctness; receivers that take more than needed
- Generic bounds on `impl` blocks that aren't actually used
- `impl Trait` in public return positions hiding a type the caller needs to name
- Constructor naming: `new`, `with_*`, `from_*`, `try_*` per conventions
- Builder chains that return `Self` vs `&mut Self` inconsistently
- Re-exports: is the public surface intentional and minimal? Flag accidental `pub`.
- Sealed trait pattern missing where future-compat matters
- `#[non_exhaustive]` missing on public enums/structs that may grow
```

### Agent F — Security

```
Review for security, weighted toward systems and confidential-computing concerns.

CHECK:
- Secrets visible in code, logs, `Debug` impls, error messages, or `Display`
- Missing `zeroize` on secret material; `Drop` that doesn't zero
- Constant-time requirements: secret comparison via `==` instead of `subtle::ConstantTimeEq`
- Deserialization (serde, ciborium, bincode, postcard) of untrusted input without depth/size limits
- Integer math: overflow in length/size/offset calculations; `as` casts that silently truncate
- Bounds: array/slice indexing on attacker-influenced indices without explicit check
- TOCTOU on filesystem, mmap, /proc, /sys
- Panic-on-untrusted-input → DoS vector
- `unsafe` crossing a trust boundary (FFI from untrusted input, sealed memory, attestation flows)
- Missing input validation at the trust boundary identified in Context Map
- Cryptographic API misuse: nonce reuse, bespoke crypto, weak RNG (`rand::thread_rng` for keys), missing AEAD
- For attestation/measured-launch code: report fields that an attacker can influence
```

### Agent G — Simplification (priority — read carefully)

```
The author's stated goal is minimal code that does exactly what's needed — no more, no less. Your job is to find big-picture simplification: API shapes that don't match actual usage, types and modules that exist without earning their weight, layers of indirection that add cost without isolation. Bias toward deletion of whole units (types, modules, traits) over rewriting function bodies.

You have Read, Grep, and Glob. Use them — the diff alone is not enough. Many of the highest-value wins live in the gap between what the diff defines and how the rest of the workspace uses it.

═══════════════════════════════════════════════════════════════
PASS 1 — USAGE MAP (do this BEFORE writing any findings)
═══════════════════════════════════════════════════════════════

For each item in the diff that is `pub` or `pub(crate)` — types, traits, enums, modules, free fns, methods — and for any non-trivial private item that looks structurally load-bearing, build a usage map:

1. Grep the workspace for callsites (`rg '\bIdent\b' --type rust`).
2. Record per item:
   - Caller count (distinct files; distinct functions)
   - Which methods / variants / fields are actually exercised vs. merely defined
   - Whether callers immediately convert away (`.into()`, `.as_ref()`, `.to_string()`, unwrap into the inner type)
   - Whether all callers pass the same generic parameter, the same enum variant, or the same trait impl
   - Whether the item is re-exported from a parent module's `lib.rs` / `mod.rs` but no external caller uses it

Keep the usage map as internal scratch — it drives the findings in Pass 2. Do not emit it.

═══════════════════════════════════════════════════════════════
KEEP RUBRIC — do NOT flag these (they earn their weight)
═══════════════════════════════════════════════════════════════

An abstraction is justified — even with one caller — if it does one of the things below AND is zero-cost or near-zero-cost at runtime (no heap allocation, no vtable, no extra indirection beyond what the underlying type already has; `#[repr(transparent)]` newtypes, monomorphized generics, and inlined trait methods all qualify).

A wrapper that costs something at runtime (Box, Arc, Vec of wrappers, dyn dispatch, Mutex around an otherwise-Sync value, async indirection over sync code) must justify its cost with more than one of the keep-list properties, or with measured benefit.

Keep-list properties:

- **Encodes an invariant via a private constructor.** Newtype that can only be built through a validating `try_new`, so the type itself is a proof. (`NonEmptyVec`, `ValidatedHandle`, `ConstantTimeBytes`.)
- **Prevents mixing of unrelated values of the same primitive.** Distinct newtypes around `u32` for `SessionId` vs `ObjectId` vs `HandleId` even if all stored as `u32`.
- **Gates `unsafe`, FFI, or a trust boundary.** Module exists to localize where unverified input becomes verified, or where a raw pointer becomes a safe reference.
- **Trait is a real contract with ≥2 implementers** (today, not hypothetically), OR a sealed trait used to constrain a generic parameter that monomorphizes to ≥2 types in practice.
- **Zero-cost wrapper enforcing a typestate** (e.g. `Builder<NeedsAuth>` → `Builder<Ready>`).
- **Module boundary that exists to keep a complex domain (crypto, parser, state machine) testable in isolation** and the boundary is non-trivially used by tests or by other modules.

If you're uncertain whether an abstraction qualifies, default to keeping it and skip the finding. False-positive deletions are worse than missed wins.

═══════════════════════════════════════════════════════════════
PASS 2 — FINDINGS (emit in this order: big wins first)
═══════════════════════════════════════════════════════════════

(A) WHOLE-MODULE OR WHOLE-TYPE REMOVAL
- Type / module has one external caller, no invariants from the keep rubric, and is structurally a thin wrapper (most methods are pass-throughs to an inner field).
- Type exists but every caller immediately converts to a different representation — the type is a way-station, not a destination. Recommend the conversion target be the actual type.
- `mod foo;` declared with `pub use foo::*;` but no out-of-crate consumer; collapse into the parent.
- Enum with N variants where workspace grep proves only M < N are ever constructed (the rest are unreachable in practice). Remove dead variants OR collapse to a struct if only one survives.
- Trait with one implementer, no `dyn Trait` use, no generic bound using it as a constraint shared across types — inline into the concrete type.

(A.5) COSTLY WRAPPERS WITHOUT ISOLATION VALUE
- `Box<dyn Trait>` field where the concrete type is known at construction and a generic parameter would inline.
- `Arc<T>` where the data is never shared across threads / tasks (single owner suffices).
- `Mutex<T>` / `RwLock<T>` around a `Copy` value where an `AtomicX` works, or around data that has exactly one writer.
- Wrapper type that forces allocation (`Vec<Wrapper<T>>` where `Wrapper(T)` could be `#[repr(transparent)]` but isn't — or where the wrapper is unneeded entirely).
- Trait object behind an interface used at exactly one callsite, where the concrete type would monomorphize cleanly.

(B) API-SURFACE NARROWING
- Type exposes N pub methods/fields, callsites only ever use M < N. Make the unused ones private or remove.
- Generic fn `fn foo<T: Trait>(t: T)` where every callsite passes the same `T`. Replace `T` with the concrete type.
- `impl Trait` return where every caller pattern-matches or names the concrete type (forcing them through workarounds). Return the concrete type.
- Builder with N setter methods where callers only use a stable subset — replace with a struct literal + `..Default::default()`, or a small constructor.
- Function takes `Option<X>` where every caller passes `Some(_)` — drop the Option.
- Function returns `Result<T, E>` where the error is constructed nowhere — drop the Result.

(C) INDIRECTION COLLAPSE
- Call chain `A → B → C` where B is a pass-through (forwards args, transforms nothing meaningful). Delete B and have A call C.
- "Manager" / "Service" / "Handler" type whose methods are 1:1 with an inner field's methods. Expose the inner field, delete the wrapper.
- Free fn that wraps a method on a type already in scope at every callsite.
- `From`/`Into` impl chain where the intermediate type is constructed nowhere except inside the chain.

(D) STATE → PURE FUNCTION
- Struct holding only references / config that's read once. Replace with a free fn taking the same args.
- `&mut self` method that never mutates `self`. Make it `&self` — or, if it doesn't read `self` either, an associated fn.

(E) IN-FUNCTION CLEANUPS (lowest priority — flag only if no A/B/C/D wins exist in the same area)
- `.collect::<Vec<_>>()` immediately re-iterated.
- Lifetime annotations that are elidable.
- `async fn` that performs no `.await`.
- Heavy crate dependency replacing 30 lines of `std`, or vice versa.

═══════════════════════════════════════════════════════════════
EVIDENCE REQUIREMENTS
═══════════════════════════════════════════════════════════════

For category A, A.5, and B findings, the `evidence_quote` field must include:
- The definition site (file:line snippet)
- AT LEAST ONE callsite proving the usage pattern (file:line snippet), OR an explicit "0 callsites found in workspace via `rg '\bIdent\b' --type rust`" if recommending removal of a never-used item

For category C findings, quote the pass-through body to show it's not doing meaningful work.

If you cannot produce the required evidence, do not emit the finding.

═══════════════════════════════════════════════════════════════
SEVERITY & LOC ESTIMATE
═══════════════════════════════════════════════════════════════

- Whole-module / whole-type removal saving ≥50 LOC: High
- API narrowing or indirection collapse saving 20–50 LOC: Medium
- Anything saving <20 LOC, or any (E) in-function cleanup: Low

In `suggested_fix`, lead with the structural change (e.g. "Delete `FooManager`; have `Bar` call `Inner::frob` directly"), then the LOC estimate (e.g. "~80 LOC across 3 files"). Synthesis ranks by LOC.

Do not propose code. Findings only.
```

### Agent H — Tests (gets coverage data)

```
Review test quality for the changed code. You ALSO receive `coverage_data` JSON from Step 2B — use it.

COVERAGE-DRIVEN RULES:
- Any function in coverage_data with `coverage_pct < 50` AND `complexity >= 5` is at least High, regardless of your other judgment.
- Any function with `coverage_pct == 0` AND any complexity > 1 is at least Medium.
- If `coverage_data.tool == "unavailable"`, note this in `not_checked` and proceed in judgment-only mode.

QUALITY CHECKS:
- Error paths tested, not only happy paths
- Boundary conditions: empty, max-size, off-by-one, overflow, UTF-8 edge, zero-length slice
- Property tests (proptest/quickcheck) where input space is large
- Tests assert behavior, not implementation details
- Async tests use `#[tokio::test]` (or equivalent) and the right flavor (multi_thread vs current_thread)
- `#[should_panic]` uses `expected = "..."` to pin the message
- For unsafe/FFI code: are tests miri-runnable? Note if `cargo +nightly miri test` would catch invariant violations.
- For public API: doctests covering the primary use cases
- Test isolation: shared state, working directory, environment variables
- Flakiness: time-dependent assertions, ordering assumptions, sleep-based sync
```

### Agent I — Correctness & Logic

```
Read the diff line by line for LOGIC bugs — code that compiles and runs but does the wrong thing. You are not checking structure (allocation, unsafe, async); you are reading for "this is just wrong."

CHECK:
- Off-by-one in indexing, slicing, ranges (`..` vs `..=`, `<` vs `<=`)
- Inverted conditions: `if x.is_some()` where `is_none()` was meant
- Swapped function arguments (especially same-type pairs: src/dst, from/to, x/y)
- Wrong field access on structs with similar field names
- Copy-paste errors: loop body referencing the outer variable, sibling match arm forgotten, last-element-of-batch dropped
- Operator precedence: `a & b == c` vs `(a & b) == c`
- Comparison against wrong sentinel: `!= 0` where `> 0` was meant for signed
- Early return that skips required cleanup or state update
- Match arms that should be unreachable but aren't, or vice versa
- Variable shadowing that silently changes meaning
- Iterator adapter that drops elements: `.zip` on unequal-length iterators, `.take(n)` where n is off
- `Option::map` vs `and_then` confusion (nested Option)
- `?` on the wrong expression (propagating the wrong error)
- Implicit fallthrough in `if let` chains

For each finding, quote the offending line AND state what the correct logic appears to be. If you cannot articulate the correct version, downgrade to a question rather than a finding.
```

### Agent J — Performance & Micro-architecture

```
Hot-path-aware performance review. Use the Context Map's hot/cold classification. Distinct from allocation (Agent B) and simplification (Agent G).

CHECK:
- `#[inline]` / `#[inline(always)]` / `#[cold]` decisions: missing on small hot helpers, present on large cold ones
- Atomic ordering: SeqCst where Acquire/Release suffices; Relaxed where Acquire was needed for synchronization
- False sharing: `Arc<AtomicX>` fields adjacent to other hot atomics
- Large enum variants forcing every value to carry the largest payload (recommend boxing the large variant)
- Monomorphization bloat: generic over a non-essential type parameter in a library hot path; `dyn Trait` could give better i-cache behavior
- Devirtualization opportunities: `Box<dyn Trait>` field where a generic parameter would inline
- `Vec::push` in tight loops where `extend` or `collect` would batch
- `format!` / `String` building where `write!` to existing buffer wins
- Trivial arithmetic loops that LLVM may fail to autovectorize
- Unnecessary bounds checks: indexed access where iterator would elide
- `Mutex` on a single-writer counter where `AtomicX` works

SKIP: cold-path optimizations, init-time code, error formatting.
```

### Agent K — Documentation

```
Review documentation quality. Distinct from API design (Agent E).

CHECK:
- Every `pub` item (fn, struct, enum, trait, mod, const) has a rustdoc summary
- Result-returning public fns have `# Errors` describing each variant
- Functions that can panic have `# Panics` describing trigger conditions
- `unsafe fn` has `# Safety` listing invariants caller must uphold
- Primary public API items have `# Examples`
- Doctests: `assert!`/`assert_eq!` actually check something (not `let _ = ...`)
- Doc claims match code: "always succeeds" on a Result-returning fn, "infallible" on a fn that can panic, "thread-safe" on a `!Sync` type
- Intra-doc links `[Item]` resolve (do not hallucinate; verify siblings exist)
- Crate root has `//!` summary if this is a lib
- `# Deprecated` paired with `#[deprecated]` attribute
- No copy-pasted boilerplate doc strings across siblings

SEVERITY:
- Missing `# Safety` on `unsafe fn`: High
- Missing `# Panics` on a fn that does panic: High
- Doc contradicts code: High
- Missing rustdoc on pub item in a library: Medium
- Missing on bin-internal pub item: Low
```

### Agent L — Dependencies & Supply Chain

```
Review Cargo.toml and Cargo.lock changes in the diff. OWNS `cargo audit`.

If no Cargo.* changes are in scope AND no advisories exist: report `clean: true` with `not_checked: ["no dependency changes in scope"]`.

CHECK:
- Every new entry in `[dependencies]`: is it justified by this diff?
- Major-version bumps: check the upstream CHANGELOG for the version range crossed
- MSRV impact: did `rust-version` change? Does any new crate's MSRV exceed ours?
- License compatibility: flag GPL/AGPL/SSPL on new crates if this project is permissive-licensed
- Run `cargo audit` and report each RUSTSEC advisory (always, even with no Cargo.toml changes)
- If `deny.toml` exists, run `cargo deny check` and report
- Cargo.lock: new transitive deps that look surprising (large, network, unmaintained); duplicated versions of the same crate
- New `[features]` entries: what do they pull in transitively?
- `default-features = true` on heavy crates (tokio, reqwest, hyper) where features could be pared
- Moving a dep from `[dev-dependencies]` to `[dependencies]` (shipping test deps)
- Build-script-heavy crates added (compile-time cost)

SEVERITY:
- RUSTSEC advisory unaddressed: Critical
- License incompatibility: High
- MSRV regression that breaks downstream: High
- Unjustified new heavy dep: Medium
- Unnecessary default features: Low
```

### Agent M — Public API / Semver Diff

```
Diff the PUBLIC API surface against the baseline. Baseline selection:
1. If a release tag exists in scope (`git tag --merged HEAD --sort=-version:refname | head -1`), use the most recent.
2. Otherwise, use the merge-base of `main` / `master`.

PROCEDURE:
1. Try `cargo public-api diff` if installed.
2. Try `cargo semver-checks check-release` if installed.
3. Otherwise: manual diff of `pub` items in the touched crate(s) — fns, types, traits, consts, re-exports, trait impls on public types.

REPORT each change classified as:
- BREAKING (semver-major): removed pub item, narrowed return type, added required generic param, tightened trait bound, removed enum variant on non-`#[non_exhaustive]` enum, changed fn signature, added required trait method
- ADDITIVE (semver-minor): new pub item, new variant on `#[non_exhaustive]` enum, new default trait method
- PATCH: internal changes only

SEVERITY:
- BREAKING in a published library without a major version bump: High
- BREAKING in an internal-only crate: Medium (still report — downstream workspace crates may break)
- ADDITIVE: Low (informational)

If this is a `[[bin]]` with no library target: `clean: true`, `not_checked: ["no public API surface"]`.
If no public-API changes: `clean: true`.
```

---

## Step 4 — Dedup & cluster (orchestrator, no subagent)

Mechanical. The orchestrator:

1. Collects JSON output from all 13 agents.
2. Sets Agent A's output aside — it bypasses dedup and critique (mechanical tool output is ground truth).
3. For agents B–M, clusters findings by `(file, line ± 3)`. Findings in the same cluster across agents are likely the same issue.
4. Assigns each cluster a stable `cluster_id`.
5. Hands the clustered findings to Step 5.

No analysis at this step. Pure grouping.

---

## Step 5 — Critic stage (batched + capped)

Parameters:
- **Batch size**: 5 findings per critic.
- **Concurrency cap**: 20 critics in flight. If >100 findings survive dedup, queue beyond the cap; warn before dispatching.
- **Grouping key**: by `category` — critics stay in one mental mode per batch.
- **Cross-batch visibility**: each critic receives its 5 findings IN FULL plus a thin index of all OTHER findings (`id`, `file:line`, `category`, `title` only) so it can flag duplicates that survived dedup.

Each critic prompt (`subagent_type: general-purpose`):

```
You are a critic evaluating code review findings. Be tough — false positives poison the synthesis. Use the unified severity rubric.

INPUTS:
- Context Map summary (verbatim)
- Severity rubric (verbatim)
- Your batch of up to 5 findings, full JSON
- Thin index of all OTHER findings (for dedup detection only)
- Read tool access to the repo

FOR EACH FINDING IN YOUR BATCH:
1. Open the cited file with Read. Read enough surrounding context to judge — not just the quoted line.
2. Verify the `evidence_quote` actually appears at the cited file:line.
3. Apply the Context Map: is this in a hot path, cold path, trust boundary, no_std crate? Does that change the severity?
4. Render a verdict:
   - "confirmed": the finding is real, severity is calibrated correctly
   - "downgraded": the finding is real but severity is too high — recalibrate
   - "rejected": the finding is wrong, overstated, or N/A in context
5. If the thin index shows another finding at the same file:line in a related category, mark `duplicate_of: <other_id>` and reject (synthesis picks one survivor per cluster).
6. Quote the source line(s) that prove your verdict.

OUTPUT JSON ONLY:
{
  "evaluations": [
    {
      "finding_id": "<id>",
      "verdict": "confirmed" | "downgraded" | "rejected",
      "recalibrated_severity": "critical" | "high" | "medium" | "low" | null,
      "reasoning": "<one or two sentences>",
      "source_quote": "<verbatim from the file>",
      "duplicate_of": "<other finding id, or null>"
    }
  ]
}

Every verdict MUST have a `source_quote`. Verdicts without one are themselves rejected by the orchestrator and the finding is dropped.
```

---

## Step 6 — Synthesize

Use confirmed and downgraded findings only. Order strictly by severity. Critical and High are SEPARATE sections.

```
## /review — <scope summary>

**Primary operations**
- <op 1 — file:line>
- <op 2 — file:line>
- ...

### 🔴 Critical  (N)
1. [Unsafe] file.rs:42 — <title>
   • Problem: <one line>
   • Fix:     <one line>

### 🟠 High  (N)
1. [Security] file.rs:120 — <title>
   • Problem: <one line>
   • Fix:     <one line>

### 🟡 Medium  (N)
...

### 🟢 Low / Nits  (N)
...

### Mechanical
- fmt:         ✓  |  ✗ <details>
- clippy:      ✓  |  N findings
- check:       ✓  |  ✗
- tests build: ✓  |  ✗

### Top simplification wins
1. file.rs:120–180 — delete <thing>; replaces with <X>; saves ~60 LOC
2. ...

### Not checked
- <dimension>: <why>
(Aggregated from every agent's `not_checked`. Includes "coverage tool unavailable" when relevant.)

### Filtered by critic
- Rejected: <N> (false positives)
- Downgraded: <N>
- Merged as duplicates: <N>
(Show titles only, collapsed. Details available on request.)

### Clean
<comma-separated agents that returned no findings>

### Verdict
**Ready to merge** | **Needs attention** | **Needs work**
<one sentence: what to do next>
```

---

## Verdict rules

| Verdict | Trigger |
|---|---|
| **Needs work** | Any Critical; OR any High in Unsafe/Security/Correctness; OR any mechanical failure (fmt/check/test-build) |
| **Needs attention** | Any other High; any Medium in Unsafe/Security; any simplification win ≥30 LOC; any RUSTSEC advisory |
| **Ready to merge** | Only Lows or nits |

---

## Orchestrator notes

- Pass agents the diff + scope + Context Map summary + severity rubric. Do NOT dump the full repo into subagent context.
- Pass Agent H the coverage JSON from Step 2B in addition to the standard inputs.
- Skip an agent only if it is genuinely N/A. Prefer self-reported `clean: true` with a `not_checked` entry over omission — synthesis stays consistent.
- If Step 5 would dispatch more than 20 critic batches, warn before continuing and ask the user.
- If the coverage tool is unavailable, Agent H still runs (judgment-only) and synthesis reports the gap in "Not checked".
- If `argv` is `all` and the workspace is large (>50k LOC), warn before dispatching and offer `branch`/`staged` as alternatives — coverage in particular will be slow.
- If you label more than 3 things Critical, recalibrate down — Critical means "merging this risks production safety / UB / security".
- No proposed patches. Findings only. The author asked for review, not refactor.
