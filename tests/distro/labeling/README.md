# Labeling admission regression

This fixture prints a marker only after production setup admits the command.
It does not itself perform xattr operations; those are checked separately by
`tests/distro/label-operations`.

Run through `tests/distro/run.py`, adding `--without-label-policy` for the negative
case. The build runner correctly returns failure for that case. Then run:

```
python3 tests/distro/labeling/verify_missing.py RUN_DIRECTORY
```

The maintained assessor requires the trusted failure status, the specific label
denial diagnostic, no command marker, and complete command/runtime cleanup.
Missing evidence fails assessment. It can run against repository-preserved logs
and status files without the large VM images. It rejects the original diagnostic/
cleanup failure in `label-admission-off-v1`; it does not reclassify it as a pass.
