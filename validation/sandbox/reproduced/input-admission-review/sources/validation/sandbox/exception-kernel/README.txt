Kernel interpretation and cleanup for small exception-record fixtures
=====================================================================
Reproduce:
  python3 validation/sandbox/guest.py exception-kernel --kernel-version 7.2.10-200.fc44.x86_64 --out /tmp/NEW-exception-kernel --run

Disposable QEMU guest, no host shares, attached host disks or network. Seven
16-KiB COW fixtures have correct blob/verity hashes and a valid snapshot header.
Each changes one exception field. Helper assembles via actual Chain::assemble,
reads only two logical 4096-byte blocks, drops its handle and requires no DM/loop
residue before the next case. Each read must be a known fixture-byte class or
record an I/O error; no filesystem is mounted and no image code runs. Helper has
30-second timeout inside 150-second guest bound. Source bytes remain unchanged.

Observed on stock 7.2.10-200.fc44.x86_64; all seven assembled and cleaned up:
  Case                    Read block 0        Read block 1
  valid-control           data-A              data-B
  terminator-old-ignored  data-A              data-B
  metadata-as-data        exception-metadata  data-B
  data-past-store         EIO                 data-B
  origin-past-64g         zero-origin         data-B
  duplicate-origin       data-B              zero-origin
  duplicate-data         data-A              data-A
A = 4096 bytes of 0x53, B = 4096 bytes of 0x71. Metadata bytes belong only to this
owned fixture. The outside-origin exception is not addressed by either read;
its acceptance is not a successful out-of-device read. Duplicate cases are
noncanonical characterizations, not an assertion that every snapshot definition
forbids them. The wire terminator legitimately ignores its old_chunk field.

Evidence: reproduced/exception-kernel. The userspace-only companion remains at
reproduced/exception-metadata. Assembly success does not imply semantic rejection
of invalid-offset records. One case defers failure to read, and another exposes
its own metadata as logical data. No kernel memory or unrelated storage was read.

This supplies bounded exception interpretation/read-error/cleanup evidence, not
exhaustive parser qualification. The termination follow-up below adds two boundary cases. No overflow-size or
resource exhaustion matrix, writable repair, malformed filesystem parser, publication
failure path or broader kernel-version claim is made. The trusted-kernel model
is unchanged. A stricter image-format admission policy requires explicit semantic
checks; these diagnostics do not implement such a policy or establish a kernel
security defect. Future reproduction requires the recorded exact read outcomes.

Missing snapshot exception terminator: bounded area-boundary controls
=====================================================================
2026-10-21 UTC. Same exception-kernel command, now nine cases.
The original seven read outcomes remain required. Two cases fill one4096-byte
metadata area with256 records. Each maps a distinct origin chunk0..255 to the
same known data chunk2; no arbitrary device address or external bytes are used.

full-area-eof: the COW stays16KiB. All256 exception slots have nonzero new_chunk,
so the metadata scan needs the next area, absent beyond this owned store. The
blob and verity hashes are recomputed correctly. Chain::stage authenticates
and retains the exact bytes, but Chain::assemble returns EIO for scute0. No
logical reads occur. DM/loop cleanup succeeds before the following positive.

full-area-next-terminator: the COW is extended with zeros through chunk258,
the next metadata area (259 *4096 bytes total, about1MiB). Its first new_chunk
is zero. Authentication and assembly succeed. Both bounded logical block reads
return the known data-A bytes as specified by their exception records. Input
bytes remain unchanged and cleanup succeeds.

The matrix therefore requires eight assemblies, one specific EIO rejection,
nine cleanups and all exact read outcomes. Helper bounds remain30 seconds with
whole guest150 seconds; actual expanded guest run completes in about2.3 seconds.
No filesystem is mounted, no image command runs, no write is made to the
assembled device. These fixtures are data parser tests under the trusted-kernel
model, not exploit code or a claim of a kernel security defect.

The exploratory run's guest completed all cases with exit0, but its outer runner
still required seven assemblies/cleanups and correctly failed that stale count.
It is retained as exception-termination-exploratory, not a passing runner result.
The corrected runner and helper now require the observed new boundary outcomes;
a fresh run passes and is retained at exception-termination.

This closes the bounded missing-terminator-at-first-area case and next-area
positive control. It does not establish general semantic admission validation,
all malformed layouts/overflow/resource limits, writable repair, later metadata
area boundaries, other kernels or full build failure propagation. A correctly
hashed input can still be structurally invalid; staging authentication does not
replace kernel assembly checks or a future explicit format validator.
