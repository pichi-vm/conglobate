# Two-root bootstrap regression

Run this plan with `tests/distro/run.py` and the qualified Fedora tool cache.
Then run `python3 tests/distro/bootstrap-labeling/verify.py RUN_DIRECTORY`.
The independent assessment requires Python cbor2.

Both commands pass through the production restricted labeling probe for `/`
and `/mnt`. The second step checks data, numeric ownership and permissions
across remount. The exported graph must contain only one newly produced scute,
no tool firmware, and a legacy trusted formatter receipt. The
[locked target](../../../DESIGN.md#11-artifact-and-build-annotations) removes
execution receipts; the follow-up code tip must retain filesystem checks and
replace receipt assertions with image-manifest annotation checks. This is a
minimal filesystem lifecycle test, not a bootable distro or complete sandbox
qualification.
