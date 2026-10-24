# M7 final qualification gate

Reviewed production source at 5d1f358 (unchanged in d50530a and the final NixOS
recipe/evidence changes). Earlier component/source reviews are reused; no broad
review campaign or repeated full test campaign was launched. Confidential
computing qualification is explicitly deferred by the user. This acceptance
covers the standalone binary and recorded environments, not an attested appliance.

## Pass 1: requirements, pipeline and style

Trace current DESIGN/SANDBOX-DESIGN/BUILD-FLOW through Pipeline and StageOutput,
immutable committed inputs, verified local cache, sealed storage and command
owners, step execution, transforms and publication. M1–M6 records cover ownership,
sealed state traits, consuming cleanup and safe Rust. The only subsequent
production changes are bounded PTY ioctls and caller-local PR_SET_PDEATHSIG;
pty-compatibility records their focused three-pass gate and failure evidence.
No generic workflow engine, network allocator, Git checkout, policy registration,
guardian or pichi binary dependency was reintroduced. One scute per carapace stage
and environment-owned identity/network/label policy remain. Clean.

## Pass 2: failures, original review and authority

Use all 111 historical finding dispositions, their current-source supplements,
linear-lifecycle review/Claude consensus and M5/M6 reviews. Historical
remaining_action fields are immutable history, not the current defect queue.
The pending hosted-CI and architecture qualifications are now covered below.
security-004 retains the source-grounded trusted-kernel compatibility decision:
splice/tee/vmsplice remain allowed; there is no established authority violation
or requirement to remove them. Fedora/APT/APK/Nix workloads now supply the
requested representative compatibility evidence. This does not assert every
allowed syscall is necessary or vulnerability-free. Trusted formatter execution
remains a declared trust boundary, not an untrusted-command escape finding.

Existing negative admission, capability/namespace restriction, blocked I/O,
termination, cleanup ownership, private resolver restoration and failed-output
exclusion evidence remains applicable. Actual failed Nix runs independently
show cleanup complete/output absent; successful current Nix processing crosses
the complete pipeline. No unresolved required implementation defect identified.
No code/style change or new external approval claim. Clean.

## Pass 3: evidence, scope and preservation

Representative builds/boots: Fedora v3 DNF/foreign-policy labeling/enforcing boot;
Debian v2 APT/systemd; Alpine v2 bootstrap/APK/musl/OpenRC; NixOS v6 pinned closure
build/export/systemd activation and numeric ownership. Their recorded source
versions remain explicit; older successes are not presented as fresh runs.
Firmware v1 constructs an initramfs and new arma PMI/DTB and boots it. Kernel
compilation and every distro/architecture cross-product are not claimed.

Hosted CI at 5d1f358: 40052820789 rootless Rust tests/doctests/Clippy/format on
native amd64 and native Arm64 plus dependency policy/advisories; 40052820734
seven lifecycle cases pass on both architectures. amd64 uses KVM. Arm64 uses
TCG on a native Arm64 runner with the real Ubuntu Arm64 kernel, explicitly
recorded in each runtime.json. This validates functional architecture paths,
not hardware acceleration or confidential computing. Earlier EFI packaging
failure is preserved and repaired by bounded zboot extraction.

No test was silently skipped to obtain these results. Full VM histories,
source/context snapshots, raw failure/success evidence and artifact identities
remain retained. M8 must verify their recoverability before retirement.
Clean.

M7 closes at three consecutive clean self-review passes. These are not three
independent external reviews. M8 remains required before goal completion.
