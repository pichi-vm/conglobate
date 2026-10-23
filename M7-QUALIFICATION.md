# M7 integrated qualification

M6 is accepted with three clean self-reviews on
preservation/m6-review/scope-v1.json. M7 is active with zero adherence passes.
The full goal and DESIGN.md/SANDBOX-DESIGN.txt remain authoritative.

## Required outcomes and current evidence

| Requirement | Current state | Acceptance evidence still required |
| --- | --- | --- |
| Source-grounded critical review | Initial Claude review and three reconciliation rounds completed; accepted fixes and unresolved dispositions preserved in preservation/m7-review | Final review, source-grounded disposition, fixes and agreement on disputed findings |
| Fedora build and boot | Sibling fedora repo and its .build directory exist; existing recipe uses mkosi | New conglobate recipe runs package transactions, generates boot firmware, emits an image that boots with its own SELinux policy |
| Debian build and boot | No sibling Debian image repo discovered | Maintained conglobate recipe, package/firmware build and boot evidence |
| Alpine build and boot | No sibling Alpine image repo discovered | Maintained conglobate recipe, package/firmware build and boot evidence |
| NixOS build and boot | No sibling NixOS image repo or host nix executable discovered | Maintained conglobate recipe, Nix build and boot evidence; host nix is not inherently required |
| amd64 | Local x86_64 QEMU/KVM integration evidence exists | Representative distro qualification and complete repository checks |
| arm64 | qemu-system-aarch64 installed | Native build/execution profile and boot coverage; executable availability proves no qualification |
| Intel TDX, AMD SEV-SNP, Arm CCA | Hardware access has not been established | Separate evidence for each platform; missing access remains outstanding |
| CI | Rootless and dependency jobs pass on GitHub (run 39964805125, 148ef66) | Portable disposable-VM integration, commit-message checks and green actual workflow results |
| Final M7 adherence | Not started | Three complete consecutive clean passes after required work |

## Initial environment/source inspection

- Current host architecture: x86_64; kernel release 7.2.11-dm-bufio-test1-dirty.
  This is not a confidential-hardware qualification.
- claude, gh, podman and both QEMU executables are installed. ORAS used by the
  integration fixture remains separately pinned under /var/tmp; no global oras
  executable was found. The pichi local artifact directory exists; its usable
  image contents have not yet been inventoried.
- The user's custom Claude Code review command is
  /home/nathaniel/.claude/commands/review.md. It requests parallel review and
  source-grounded critics. Its full instructions must be read before invocation;
  the M7 invocation, raw stream and parent evaluation are retained under
  preservation/m7-review. The original launcher denied some shell checks through
  its narrow allowlist; these are permission denials, not a broken shell or
  successful checks. The follow-up launcher permits Bash while retaining the
  explicit review constraints. The separate rust-release-review skill
  targets publishing and is not the selected workflow for this rewrite.
- Sibling Fedora build.sh produces a reusable tool/base image with mkosi, corium
  and arma. It must not be counted as a conglobate-produced distro image. Its
  existing boot tests are reference inputs, not proof for the replacement pipeline.
- The old CI invokes ignored privileged tests directly under sudo/unshare. Do not
  restore that lane verbatim: all new privileged validation belongs inside
  disposable VMs. New CI must install the BPF build dependencies used by build.rs
  and supply a qualified VM kernel, not silently skip required runtime coverage.
- Local stable-toolchain checks now include Rust/Cargo 1.94.0. The first strict
  Clippy run found a redundant inventory-validation boolean expression; the
  equivalent simplified expression passes strict Clippy. Both logs are retained.
  The rootless workflow is only one part of the required CI coverage, and has
  started hosted run 39964104257 on rewrite-qualification. Its Rust job failed
  because the BPF target lacked the Ubuntu multiarch UAPI include path; the
  corrected workflow passes the Rust job in run 39964805125 at 148ef66;
  dependency checks also pass. Later commits require their own hosted checks.
- The VM runner now builds its own fresh guest runtime from trusted local tools
  and accepts explicit kernel, module-root and ORAS paths. It no longer copies
  a previous experiment's guest or QEMU command. v256/v257 preserve packaging
  failures (module-path normalization and host modprobe configuration); v258
  passes the full existing suite with the new runtime. Inputs, serial logs,
  source archive and provenance are under
  validation/rewrite-command-integration/evidence/. CI provisioning of a
  qualified kernel/module/tool set is still required.

## Execution order

1. Run the custom critical source review against active production source/tests
   and the agreed designs; exclude preserved legacy implementation as review
   target. Evaluate findings against actual invariants and negotiate disagreements
   with Claude. Preserve raw findings and dispositions; no blind acceptance.
2. Restore repository checks and a portable disposable-VM runner using explicit
   kernel/tool inputs. Keep cargo builds serialized. Establish checks locally,
   then actual CI; no claim of green CI from local logs alone.
3. Inventory locally available verified tool images and cached bootstrap inputs.
   Adapt the existing Fedora workflow into a committed single-file conglobate
   recipe and lock, then build and boot its actual output in disposable VMs.
   Bootstrap tooling acquisition belongs to fixture/environment preparation;
   conglobate itself never downloads missing artifacts.
4. Add Debian, Alpine and NixOS image contexts following the same interface.
   Exercise their actual package managers and firmware construction rather than
   substituting tiny synthetic roots for distro qualification. Preserve resulting
   manifests/inventories, console logs and metadata/label checks.
5. Qualify the supported architecture and confidential-platform matrix where
   access exists. Report missing access precisely and keep that coverage open;
   continue other meaningful work rather than treating it as a global blocker.
6. Resolve findings, complete M7 adherence gates, then perform M8 recoverability
   verification and retirement. Existing validation/legacy files remain intact.

## Syscall test coverage boundary

The current VM fixture demonstrates ordinary thread creation and rejection of
`unshare --user` under command restrictions. It does not exercise the `clone`
argument-mask rule: unshare is a different syscall and is default-denied. A
clone-specific control with namespace-creation authority before applying the
unchanged filter remains required. See the round-three parent evaluation; neither
EPERM alone nor ordinary thread creation distinguishes capability denial from
the namespace argument filter.
