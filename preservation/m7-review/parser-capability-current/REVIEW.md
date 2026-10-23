# Parser and capability regression batch

Based on 48c6662. Application behavior unchanged.

- tests-002 (revalidated fixed): The rootless capability contract now excludes SYS_ADMIN, SETPCAP, SYS_MODULE, BPF, NET_ADMIN and NET_RAW independently of the retained allowlist. This checks policy construction, not kernel application; VM tests remain necessary.
- tests-008 (partially addressed; additional budget coverage outstanding): Reconsidered the historical blanket rejection: default budgets do not by themselves supply wrapper regression coverage. A small 8-level successful control and 80-level nested sequence verify depth rejection through YamlDocument::decode. Alias-expansion and scalar-budget coverage remain outstanding; no general resource-exhaustion proof claimed.
- performance-010 (revalidated fixed): SourcePattern::matches allocates two state vectors before the token loop, clears and swaps them per token. No per-token vector allocation remains. This is source evidence, not a throughput benchmark.

All 148 library tests passed. Strict all-target Clippy initially rejected test-module placement; moved the module below production items, then the same check passed. Both logs are retained. No privileged host tests or new VM runs. No full milestone pass awarded.
