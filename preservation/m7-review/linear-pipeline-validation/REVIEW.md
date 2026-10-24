# Consolidated linear-pipeline validation

Production revision: 0556f30. Both guest archives' production source files match
that revision (see production-source.json). The fixture is newly maintained in
tests/distro/pipeline. No production code changed to make this test pass.

The initial fixture failed: the seed lacks awk, and the newly populated root
lacked /dev, /proc and /sys. Command setup failed and publication was absent.
The v1 source, logs and failure record are retained. The corrected recipe uses
shell token parsing for ldd output and supplies ordinary runtime mountpoints.

The v2 full CLI build passes bootstrap -> carapace -> firmware -> transform ->
assembly/inventory -> publication. Bootstrap supplies a minimal shell root;
three carapace directives exercise run/COPY/run persistence and temporary-file
removal. Explicit Fedora firmware tools are separate from the output root.
The verifier requires exactly two produced scutes (one bootstrap, one carapace),
the supplied PMI, a DTB whose /chosen/bootargs contains corium bound to the final
root, canonical version-two inventory and exactly the referenced OCI blob set.
The final verifier additionally checks the complete layer media-type multiset;
that host-side assertion was added after guest capture and passed on its output.

Existing Rust suite: 227 tests and 46 doctests pass. Strict all-target Clippy and
formatting pass. No full adherence gate advances from these checks. In particular,
this does not qualify all stage/failure combinations, bootability, distro package
workflows, arm64, confidential hardware, or the entire sandbox. Empty-stage and
failure/lifecycle combinations still require current-revision qualification.

Raw small evidence and source/context archives are retained here. Large guest
kernel/initramfs/export assets remain local with hashes; these are not durable
M8 archives. The full exported OCI layout is available in the recorded v2 run's
artifact directory. Old validation and user work remain untouched.
