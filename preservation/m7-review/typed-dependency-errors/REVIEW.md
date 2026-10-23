# Preserve dependency error sources

Historical errors-api-008 is resolved: TransformError::Dtb owns devtree::Error,
and CacheError::Manifest owns pichi_artifact::Error. Derived From/source replace
stringification at the existing call sites. Both pinned dependencies supply
owned error types; no new dependencies, authority or mutable state are needed.
ContextError::Git is unchanged: its distinct error/lifetime constraints were
explicitly excluded from this finding.

Malformed-input regression checks go through actual transformation and cache
resolution, then downcast Error::source to the dependency type. They verify the
same diagnostic prefix and dependency Display. Existing full suites check
normal behavior. No storage ownership, cleanup or publication transition changed.
Strict root and maintained fixture Clippy pass. Evidence does not claim new
privileged VM qualification; complete milestone adherence remains outstanding.
The wider Execution(String) diagnostic design remains a separate open item.
