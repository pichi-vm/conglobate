# Current dependency reconciliation

Base: 503fbe3. No dependency versions or executable behavior changed.

- dependencies-001 (revalidated rejection): Current cargo-audit 0.22.2 exited 0 after loading 1351 advisories and scanning 205 dependencies. This is advisory coverage, not a full dependency source audit; README states Git-source matching limitations.
- dependencies-003 (documentation repaired; pin rationale revalidated): Cargo.toml still referenced an ARCHITECTURE explanation that had disappeared. Restored it after inspecting exact Git rustix mount_setattr with MountAttr lifetime and confirming no such interface in locked registry rustix 1.1.5. Application callsites require that safe API.
- dependencies-004 (revalidated rejection): Qualified cargo tree source identities show registry rustix enters through tempfile, while application mount APIs use pinned Git rustix. Multiple-version warnings are intentional; source replacement would erase the required API distinction.
- dependencies-005 (revalidated fixed): deny.toml explicitly allows bytecodealliance/rustix and pichi-vm Git sources. Both amd64 and arm64 licenses/sources/bans checks exited 0.
- dependencies-006 (revalidated fixed): License allowlist includes BSD-2-Clause and BlueOak-1.0.0; both target-graph checks pass. This is tool policy evidence, not a legal compatibility opinion.
- dependencies-007 (revalidated rejection): Package remains publish=false and workflow pins Rust 1.94.0. No lower MSRV is promised by Cargo.toml. This batch does not establish compatibility with other compiler versions.
- dependencies-009 (revalidated rejection): Cargo.lock remains tracked; CI uses --locked for compilation and deny checks. Exact seccompiler/linux-raw-sys/netlink pins remain deliberate and are not an observed defect.
- dependencies-010 (revalidated rejection): Resolved gix features are sha1/sha256 and gix-transport has only default. Exact gix-transport source gates async_io, blocking_io and client::git behind disabled client features. Presence of the package alone does not enable transport clients.
- dependencies-013 (local checks current; hosted qualification outstanding): Current rootless/dependency workflow exists with pinned actions and checker versions. Both local target policy checks and audit pass. Historical hosted result is retained; no current-head hosted CI success is claimed.

Commands: cargo +stable metadata --locked --format-version 1; cargo deny --locked --target {x86_64,aarch64}-unknown-linux-gnu check licenses sources bans; cargo audit. Checker versions were cargo-deny 0.19.9 and cargo-audit 0.22.2. Exact source-qualified cargo tree outputs are retained; the initial ambiguous rustix query failed and is also preserved. Metadata captures resolved features and local dependency source paths. Policy warnings are retained, not suppressed. No arm64 execution, hosted run, or full milestone pass is claimed.
