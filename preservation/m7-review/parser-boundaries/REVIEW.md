# Firmware architecture and interface-name parser coverage

At d462816, accepted tests-004 and tests-006 were still missing. Production
behavior is unchanged; this change adds coverage of existing validation.

The independent PE/CBOR fixture now accepts a COFF machine field for section
construction. Its existing convenience image builder retains the x86 default.
The new ordinary-VM test exercises the AArch64 branch using ARM CPU profile and
register fields. Both EL1 PSTATE modes (4 and 5) are accepted, including detached
DTB capacity checks. Missing PSTATE, invalid modes, an x86-only register, and
crossed machine/schema combinations are rejected. Unsupported machine rejection
is checked by its specific diagnostic. CPU expectations were checked against
the pinned pmi 364ee0b source, including required PState and deny_unknown_fields.

InterfaceName tests pin the existing private-name policy: ASCII alphanumeric,
underscore and hyphen, nonempty and at most 15 bytes. Sixteen bytes, punctuation
outside that set, separators, whitespace, NUL and non-ASCII are rejected. This
tests our wrapper policy, not all names Linux can represent.

All 130 library tests and strict all-target Clippy passed. No new executable
firmware, ARM boot, hardware isolation or confidential-platform evidence is
claimed. The fixtures are deliberately structural and run on the current host.
Source hashes and logs are retained alongside this review.

Focused review only. M4 remains 0/3 complete clean adherence passes; M7 ARM and
confidential-platform qualification remains outstanding.
