# Environment-supplied identity mappings

Decision: the VM environment configures and supplies UID/GID ranges. No parent-ID
allocator belongs in conglobate. Initial interface is one contiguous range per
identity kind, image start zero, independent counts, both required for execution.
The trusted environment reserves these identities away from service accounts and
other workloads. Numeric checks cannot establish reservation; no such claim is made.

Source review:
- IdRange validates nonzero parent start/count and rejects overflow/invalid IDs.
  FromStr rejects malformed decimal syntax. Clap requires paired arguments.
- BuildPlan owns the execution predicate used by preflight and Pipeline. Captured
  values retain effective mappings only for execution. All three stage call sites
  use this pair; none supplies hidden fixed defaults.
- Existing ImageOwnership checks inode/ACL IDs against the supplied counts. User
  namespace and idmapped mount setup use the same ranges; no authority is added.
- Inventory version three records effective mappings and validates decoded ranges.
  Decoder validates representation, not truthful execution or range reservation.
- No unsafe code, independent functions, new processes, or resource lifetimes are
  introduced. Mapping values are immutable and carried by existing owners.

Validation:
- rust-tests.log: 229 tests and 46 doctests pass.
- clippy.log: strict all-target Clippy passes. fmt.log: formatting passes.
- Full native amd64/KVM offline pipeline: exit zero, all four stage markers and
  artifact export pass. UID 70000/GID 80000 persists across bootstrap sealing and
  carapace remount under environment ranges 200000:131072 / 400000:131072.
- verification.log validates deterministic inventory version three and exact maps,
  final DTB root binding, firmware selection, one carapace scute and blob graph.
- source.tar.gz is the guest's source snapshot. guest-source-difference.patch
  records final production changes since snapshot (CLI help comments only).
- context.bundle preserves the committed fixture. inventory.cbor and serial.txt
  preserve output receipts. local-assets.json hashes large assets retained locally.

Not a full milestone adherence pass, distro boot, platform qualification or
retirement authorization. Previous evidence and failed experiments remain intact.
