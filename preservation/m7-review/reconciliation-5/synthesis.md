## performance-001 — confirmed correct; one gap probed and found closed

I verified each requirement against source rather than the commit message.

**Only the duplicated scute identity scan is removed.** `verify_references_with_chain` substitutes a receipt *only* when the layer is `Layer::Scute` or `Layer::ScuteZstd` **and** the chain supplies a match. Every other arm falls through to `self.verify(...)`. The config descriptor at `resolve.rs:109-112` is untouched and still verified unconditionally.

**The substitution is sound, not a trust shortcut.** This was the load-bearing check. `VerifiedChain::receipt` only matches scutes that came out of `verify_scute`, and every such scute was streamed through `CheckedBlob`, whose `finish()` (`chain.rs:186-193`) enforces all three of: no trailing bytes, `count == size`, and `hash.finalize() == expected`. That is the same size-and-SHA-256 pair `Cache::verify` establishes. `BlobReceipt` is exactly `{digest, size}`, so the reconstructed receipt is byte-equivalent to the one `verify()` would have returned — not a weaker token.

**Receipts remain in manifest order.** The loop still iterates `&manifest.layers` in order and pushes one receipt per layer; only the *source* of each scute receipt changed. The new test deliberately reverses the layer vector (`layers.reverse(); // Receipts follow manifest order, not ancestry order`) so manifest order and ancestry order disagree, then asserts the combined receipts match the standalone ones element-wise on both digest and size. That is exactly the right adversarial shape — had `receipt()` been driven by chain order, this test would fail.

**Standalone APIs stay complete.** `verify_references` remains `pub` and now calls `verify_references_with_chain(image, None)`, so with no chain every layer takes the `self.verify(...)` path — identical to the pre-commit behavior. `verify_chain` is unchanged and still independently callable. `verify_content` is `pub(crate)`, so it adds no public surface.

**Compressed decoded-length pass stays.** `verify_scute` still runs the measuring pass through `DecodedBlob` into `io::sink()` before constructing `ScuteExtent`, then re-opens for the hashing pass. Both passes go through `CheckedBlob`, so both authenticate. The known-length verity requirement you insisted on is intact, and `data_blocks` still only bounds the measuring read — it does not replace the measurement.

**Later exact-inode preparation is untouched.** `src/cache/chain/prepared.rs` and `src/chain.rs` have no diff in this commit. `prepare` still re-opens via `cache.blob(&self.digest)`, re-checks the length, and re-derives the root against `self.root`, with the comment about retaining the same open inode intact. Your preservation requirement is met: a prior receipt authorizes nothing later.

**Callers.** `src/input.rs:157` and `src/output/assembly.rs:313` both collapse two calls into `verify_content`, preserving the `chain.is_none()` execution-carapace check in the former and discarding only the receipts (`let (_, chain)`) in the latter, which is what the previous code did too.

### Gap I probed — and it is closed

The `receipt()` lookup matches on `digest == *digest && blob_size == size`. I checked whether a manifest could list the *same* scute digest twice and get two receipts from a single verification. It cannot: `verify_chain` inserts into `candidates` keyed by root and rejects `"duplicate scute roots"`, and the ancestry walk plus the `!candidates.is_empty()` check requires every verified scute to be consumed exactly once in a linear chain. A repeated digest would produce an identical root and be rejected before `receipt()` is reachable.

I also checked the inverse: a scute listed in the manifest but *not* in the returned chain (e.g. ancestry-disconnected). That cannot reach `receipt()` either, since `verify_chain` errors out rather than returning a partial chain — and if it somehow did, `and_then` yields `None` and the layer falls back to a full `verify()`. The fallback is fail-safe in the correct direction.

**No concrete flaw found. performance-001 → fixed**, with the scope you specified: the duplicated *initial* scan is gone; exact-inode authentication at preparation is preserved.

### Evidence scoping

I ran no Cargo commands, per your instruction — the VM runner is building. I am relying on your preserved `combined-verification-*` logs for 200 rootless tests, 54 doctests, strict Clippy and fmt; those are parent results, not mine. **v263 has no verdict, so I claim no VM pass for this optimization.** v262 passed its full suite but predates this commit, so it does not cover it. The new `combined_verification_retains_every_receipt_and_rejects_changed_content` test is rootless and covers the corruption-rejection property for all six blobs (two scutes, pmi, dtb, config) through both the combined and standalone APIs.

### Per-ID updates

- **performance-001 → fixed.** Duplicated initial scute identity scan removed via receipt reuse; ordering, non-scute coverage, standalone APIs, compressed length pass and preparation authentication all verified intact.

No other ID changes this round. I'm not restating aggregates — my round-three and round-four totals were arithmetically wrong (112, not 111), and I won't quote a total again until it's recomputed from a validated array of the 111 unique IDs.

### Standing items unchanged

- `0x80` / `CLONE_NEWTIME` remains uncovered — it aliases the legacy clone exit-signal byte, so the probe cannot exercise it as a time-namespace control.
- Round-four mechanism consensus recorded; v262's full-suite pass and preserved provenance noted, scoped to commits at or before its archive.
- No host privileged operations, no probe execution, no edits. I read active source under `src/` and `validation/.../run.py`, not archived logs or the extracted v262 source tree.
