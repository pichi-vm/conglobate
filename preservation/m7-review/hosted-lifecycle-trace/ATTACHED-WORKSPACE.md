# Attached workspace alternative: source assessment

The user challenged whether detached-subtree cloning is necessary. It is an
implementation dependency, not a command-isolation requirement. This assessment
supersedes treating HWE as the settled compatibility solution in REVIEW.md.
The earlier failure evidence remains valid.

Current dependency sites:

- Runtime::create clones its control subtree from Workspace's detached tmpfs.
- StagedContext::read_only clones a workspace-backed checkout, before any stage
  Runtime is created (Pipeline::run). Fixing Runtime alone is insufficient.
- ResolverSource::mount clones a workspace-backed file for read-only delivery.
- Volume::view clones an already attached volume; command root setup clones an
  attached root. ProcRestrictions installs restrictions after attaching procfs.
  Those sites do not independently require detached-source cloning.

Candidate: attach the single tmpfs beneath an exclusively created, root-owned
0700 directory in the deployment init mount namespace before materialization.
Keep NODEV/NOSUID/NOEXEC and private propagation. Its owner must retain a pinned
parent, mount descriptor and exclusive target identity, explicitly report failed
unmount/removal, and prevent publication after unsuccessful teardown. It must
retain the RAM-only policy and avoid a second scratch filesystem. Consider
whether runtime control can live directly under this root rather than retaining
the now-unnecessary subtree clone; preserve recovery directory semantics.

Do not casually unshare the supervisor: InitMountNamespace and RecoveryDirectory
explicitly require deployment init's mount/PID namespace. Command setup already
creates a separate mount namespace, makes propagation recursively private,
pivots into the assigned root and detaches the old root. EnteredRoot audits
remaining descriptors before exec. These are source-supported mechanisms, not
proof that the proposed attachment is isolated in every failure path.

Required disposable-guest validation before adopting the candidate:

1. On the previously failing 6.8 kernel, verify read-only checkout, runtime
   creation/closure and resolver mounting from the attached workspace.
2. Exercise ordinary success, setup failure, command failure, timeout and cleanup
   failure. Verify no mount or owned attachment directory survives successful
   teardown; failed teardown prevents publication and retains recovery ownership.
3. Verify commands cannot resolve a supervisor workspace sentinel by pathname,
   proc descriptors or old root; descriptor audit and existing confinement tests
   must still pass. Verify private propagation and namespace-local changes.
4. Verify the filesystem remains tmpfs, inherited layers remain cache-backed,
   backing devices remain hidden and workspace destruction cannot precede leases.
5. Run maintained lifecycle and full pipeline tests, then redo affected adherence
   gates. New mount ownership affects M3 workspace/context and M4 lifecycle; do
   not retain those clean gates by claiming this is merely CI configuration.

No attachment implementation or safety qualification is claimed by this note.
Current HWE run 40039827402 remains a baseline experiment, not a final decision
that older kernels are unsupported. The next implementation task is an owned
attached-workspace lifecycle with fallible close, including error paths.
