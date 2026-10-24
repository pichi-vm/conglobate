# M2 architecture adherence — pass 3, finding; streak reset to zero

Scope 8fdf264, evidence-first self-review. The maintained lifecycle README still
claimed protected command records remained after helper death and were removed
by cleanup. Source contained.rs asserts the lease directory is empty even while
the cgroup remains populated. gate_failure.rs uses drained domain ownership,
not a retired record. These claims contradict the agreed no-journal architecture.
Corrected the README; historical evidence and fixture directory names remain.
The first two clean reviews remain historical and do not permit advancement.

The rest of the review traced requirement-to-test anchors back to immutable plans,
stage ownership, typed format profiles, restricted CommandPrivileges, error owners
and publication. The schema and coverage caveats are consistent; no unsafe or
new procedural framework was added. Missing distro/platform/failure qualification
remains explicit. No preservation data was removed. Restart all three reviews on
the corrected scope; do not count compilation or repeated tests as reviews.
