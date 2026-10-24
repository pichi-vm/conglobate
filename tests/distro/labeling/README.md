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
denial diagnostic, no command marker, complete command/runtime cleanup, and the
trusted runner's assertion that no output directory exists.
Missing evidence fails assessment. The assessor consumes generated logs and
status files without requiring the large VM images.
