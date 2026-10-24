# M7 integrated qualification

M4–M6 are accepted at three focused self-review passes; current records are
`preservation/m7-review/linear-lifecycle/ADHERENCE.md`,
`linear-bootstrap-adherence.md` and `linear-pipeline-adherence.md`. M7 remains
active with no qualification gate passes. Reuse existing evidence; rerun only for
changed behavior or a concrete unresolved finding. The long external M4 sweep
was interrupted, its limited synthesis and source-grounded consensus preserved;
it is not independent approval of the entire implementation.

Hosted rootless and lifecycle CI both pass at 9a70e50 (40049514957 and
40049515059). Alpine bootstrap/APK/OpenRC build and boot now pass on native amd64;
its maintained recipe is in `tests/distro/alpine`. Earlier distro runs below
remain evidence at their recorded source versions, not fresh current-source runs.

## Required outcomes and current evidence

| Requirement | Current state | Acceptance evidence still required |
| --- | --- | --- |
| Source-grounded critical review | Initial Claude review and four reconciliation rounds completed; accepted fixes and unresolved dispositions preserved in preservation/m7-review | Final review, source-grounded disposition, fixes and agreement on disputed findings |
| Fedora build and boot | Fedora v3 passed real DNF installation, root labeling and inherited-firmware boot; guest service verifies enforcing mode, expected label and UID 0 | This covers the representative layering/SELinux case on amd64; other distros, real bootstrap/firmware-generation workloads and platform coverage remain separate |
| Debian build and boot | Native amd64/KVM representative APT build and boot passed; `preservation/m7-review/debian-build-v2` preserves sentinel, package records and artifact hashes; v1 volatile-/var failure retained | Debian kernel generation and broader architecture/confidential coverage remain outstanding; this case packages the known Fedora-built kernel/corium pair |
| Alpine build and boot | Native amd64 bootstrap/APK/OpenRC build and boot passed; evidence in preservation/m7-review/alpine-build-v2 | Alpine kernel generation and other architectures/platforms remain outstanding |
| NixOS build and boot | No sibling NixOS image repo or host nix executable discovered | Maintained conglobate recipe, Nix build and boot evidence; host nix is not inherently required |
| amd64 | Local x86_64 QEMU/KVM integration evidence exists | Representative distro qualification and complete repository checks |
| arm64 | Native ubuntu-24.04-arm CI added | Native rootless checks and lifecycle guest results pending; TCG acceleration, if needed, is explicitly recorded |
| Intel TDX, AMD SEV-SNP, Arm CCA | Deferred by explicit user direction | Not part of this implementation gate; no confidential-platform qualification claimed |
| CI | Rootless and disposable-VM lifecycle jobs pass at 9a70e50 | Later source changes require appropriate checks; broader platform qualification remains separate |
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
argument-mask rule: unshare is a different syscall and is default-denied. The v262 disposable-VM run adds a direct legacy-clone probe: all seven supported
namespace flags succeed with namespace authority and no_new_privs before filtering,
then fail with EPERM under the unchanged filter while ordinary clone still works.
Both arms retain effective SYS_ADMIN. This distinguishes the argument filter from
capability denial. The full v262 suite passed; independent review accepts the mechanism. No time-namespace
control is claimed because legacy clone uses that bit as part of its exit signal.
