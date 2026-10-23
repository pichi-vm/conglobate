# Bootstrap command rejection

Run recipe.yaml through tests/distro/run.py with the qualified Fedora tool cache.
An exit status of 1 from the runner is expected. Assess the result with:

```
python3 tests/distro/failures/verify.py RUN_DIRECTORY bootstrap-nonzero
```

The first command writes and reads both tool and output roots before exiting 7.
The assessor requires that marker, the command exit diagnostic, cleanup completion,
no later-step marker, and the trusted runner's output-absence assertion. This
tests rejection after admitted two-root execution, not only pre-admission failure.
It does not enumerate every kernel resource or qualify a bootable distribution.
