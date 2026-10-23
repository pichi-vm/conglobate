# Explicit setup deadline API cleanup

Historical finding concurrency-003 is addressed for the three convenience APIs:
CommandSetup::enter, GatedCommandSetup::enter and CommandNamespaces::enter.
Each previously selected Instant::now() + 30 seconds. None had a caller in
current production or maintained tests. Removed these wrappers; existing
enter_until and internal enter_with_network retain the supplied absolute
cutoff. Their implementations and actual helper execution path are unchanged.

GatedCommandSetup::execute still derives its deadline from SetupCutoff and
checks that cutoff before spawn. GatedRootSetup::restrict checks before and
after root transition. This is an API tightening, not evidence of a prior
reachable production timeout-renewal vulnerability. A public Instant argument
still relies on its caller supplying the correct cutoff; do not claim the
signature alone proves deadline provenance.

The rootless suite, doctests, strict root and maintained-fixture Clippy and
format check pass. No new VM run is claimed or needed for removing unused
wrappers; prior evidence retains its original source identity. Changes affect
the review scope, which remains at zero complete M4 adherence passes.

Historical validation code still references old convenience methods. It was
already incompatible with removed policy APIs and remains preserved unchanged.
When migrating those specific fixtures, supply their explicit setup cutoff.
Do not restore implicit deadlines merely to compile a historical harness.
No freestanding functions, mutable state flags, unsafe code or extra authority
were introduced. Legacy/public consumer reconciliation remains broader than
this finding and is not declared complete.
