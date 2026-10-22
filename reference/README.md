# Rewrite references

`legacy/` is the pre-rewrite implementation, including the previously uncommitted
changes. It is not part of the new implementation's build. Its original layout
and all versioned validation evidence are recoverable from Git tag
`rewrite-reference-2026-10-22` (commit 7bd1863).

Exploratory validation remains at ../validation/ so existing evidence links stay
readable. Treat it as precedent, not maintained coverage for the new code. Some
harnesses depend on the original checkout layout or external temporary paths;
run the tagged reference checkout when reproducing them, not this relocated tree.

The exact pre-move byte archive, including nested generated helper caches, is
recorded in ../preservation/BASELINE.txt. Do not remove references until milestone
8 has migrated important findings and verified recoverability. No root Cargo
package exists between separation and the new foundation milestone; the legacy
workflows have likewise been moved out of the active CI path deliberately.
