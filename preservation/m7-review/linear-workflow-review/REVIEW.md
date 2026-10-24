# Linear workflow source review

Reviewed the complete request-to-publication path at 26023ad and the resource
acquisition corrections in this commit. This is a structural review, not one of
the required complete milestone adherence passes. Prior changes reset the gates.

| Boundary | Source-grounded assessment |
| --- | --- |
| Environment inputs | CapturedBuild reads verified committed bytes and captures external network inputs; no clone, checkout or NAT provisioning. Removed a stale private-materialization comment. |
| Local artifacts | CapturedBuild::resolve verifies every locked image's content and rejects missing execution roots before privileged execution. |
| Stage order | Pipeline::execute sequences bootstrap, carapace, firmware. No stage runs another stage. |
| Layer granularity | DerivedCarapace::derive creates one snapshot, passes the full directive slice to CommandSandbox, and seals once only after success. Empty directives return unchanged. |
| Step isolation | CommandSandbox::directives_with_network prepares a receiver/storage for each run, selects its network mode and timeout, and obtains unmounted storage back before continuing. COPY uses its confined storage path. |
| Output ancestry | Only carapace.from supplies inherited components. Bootstrap tools and explicit firmware tools do not become the output base. |
| Root identity | Found unnecessary assembly for identity-only use. Corrected: map only for carapace directives or implicit firmware tools; derive CARAPACE from sealed or verified roots. |
| Pass-through | Found unnecessary workspace creation. Corrected: inherited-only finalization uses no workspace or execution runtime. |
| Firmware/DTB | FirmwareTools captures files after command cleanup. StageOutput finalizes the pair or transforms inherited DTB after Runtime::scoped closes successfully. |
| Publication | All paths reach one publish call after assembly and inventory validation. Runtime failure prevents reaching finalization. Output staging uses no-replacement publication. |

Open qualification, not implied by compilation:
- Multi-step COPY/run produces exactly one new scute; empty stages produce none.
- All allowed bootstrap/carapace/firmware combinations, explicit versus implicit
  tools, firmware-only output and missing CARAPACE behavior on current source.
- Failure paths cannot publish; per-step isolation survives snapshot reuse.
- Full Rust/style/ownership/security adherence passes, not just this flow review.
- Current distro boots, arm64/confidential platforms, durable reference retirement.

The workspace remains conglobate-owned detached tmpfs for executed builds.
Environment-owned workspace input was proposed but has not been adopted. The
existing newer-kernel subtree-clone dependency remains an implementation concern;
this change does not claim to resolve it. UID/GID range configuration remains
unresolved. Do not infer decisions from either omission.

Validation in this batch: cargo check --locked --all-targets and formatting;
no new VM, distro or broad test campaign. Earlier evidence is revision-specific.
