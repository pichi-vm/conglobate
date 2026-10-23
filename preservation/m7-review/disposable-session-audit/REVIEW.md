# Disposable-session simplification

Latest user direction: keep the implementation simple and straightforward.
The current persisted cgroup record protocol is unnecessary for a dedicated VM
that is destroyed on supervisor failure. Live cgroup descriptors already supply
joining and cancellation authority. Remove record serialization/publication,
reopening and record-cleanup phases; preserve actual descendant quiescence,
bounded teardown, mount cleanup, writeback checks and publication exclusion.

First source cleanup removes three uncalled reopening methods: recover_domain,
recover_record and DomainCancellation::recover. Tests (232 plus 53 doctests),
strict Clippy and formatting pass; the maintained lifecycle fixture also compiles.
This does not yet remove the active record protocol. That is the next coordinated
change across domain/helper/drain and step preparation/teardown. Protected mount
ownership and namespace identity checks need their own audit; no safety claim
is inferred from deleting unused APIs. M4 remains 0/3.
