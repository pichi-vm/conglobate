# Remove setup-cleanup record-path dependency

StoragePreparationFailure no longer borrows or forwards the domain parent path.
Its resource owners already retain the needed descriptors. Removed the extra
lifetime parameter, launch path argument, and now-unused CommandSandbox accessor.
Actual CommandDomain creation still requires the protected cgroup parent.
Removed one compile-fail example enforcing the obsolete pathname lifetime;
retained compile-fail checks against extracting live storage or renewing deadlines.

227 tests and 46 doctests pass; strict root/lifecycle Clippy and formatting pass.
An initial mechanical edit matched the parent suffix of domain_parent, causing
compile errors; corrected the create parameter and removed only launch's obsolete
parameter. Initial Clippy also identified the unused accessor, now removed.
These were build-time edits, not executed privileged failures. Existing VM
containment evidence is not claimed as a new run on this exact source. Full
milestone gates remain pending. Workspace ownership question is outstanding;
no environment-supplied workspace contract is implemented or assumed here.
