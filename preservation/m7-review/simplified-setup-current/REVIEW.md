# Setup cleanup after record and path removal

Production source is 5204d7d. Initial native Fedora 7.2.10 disposable-VM batch
passed namespace and mount failure rollback. Launch case failed before launch:
the fixture still expected one serialized recovery record, now intentionally
absent. Join case was not reached. Raw failed evidence is retained under v1.

Updated all former live-record assertions in sandbox_failures.rs to inspect the
actual cgroup directory (one directory, cgroup.type domain) and separately assert
zero records. Existing post-cleanup checks still require no cgroups, no volume
mount directories and no child processes. No production behavior changed.

The v2 batch passes launch failure, join failure and attached helper exit. Each
case exits zero and powers off. The runner records exact source/inputs and raw
results. The additional blocked/timeout cases sharing updated assertions have
not been rerun here and are not claimed as newly qualified. Fixture strict
Clippy passed. No milestone adherence gate advances.

Hosted lifecycle run 40041269726 independently passed its four maintained cases
at earlier 1523bc1; its coverage is not this later source. Large local guest
assets have hashes here but are not durably archived for M8. No exploratory
validation is discarded. Workspace ownership remains awaiting a user decision.
