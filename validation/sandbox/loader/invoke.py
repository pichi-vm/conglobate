#!/usr/bin/python3
"""Trusted launcher gives the helper either inherited or constructed env."""
import os
import sys

root, mode = sys.argv[1:]
untrusted = {
    "LD_PRELOAD": "/loader-canary.so",
    "LD_LIBRARY_PATH": "/fixture-no-libraries",
    "BASH_ENV": "/fixture-no-shell-init",
    "PYTHONPATH": "/fixture-no-python",
    "GCONV_PATH": "/fixture-no-converters",
    "EVIL_SETUP": "fixture",
    "PATH": "/fixture-no-tools",
    "HOME": "/fixture-no-home",
}
# The Python invoker's own environment is trusted. The canary is passed only
# to execve of the helper, so it cannot run in a preceding shell/Python loader.
env = untrusted if mode == "inherited" else {"PATH": "/usr/bin:/usr/sbin", "HOME": "/"}
assert mode in ("inherited", "sanitized")
os.fstat(9)  # Positive control: closure must happen in the helper, not earlier.
os.execve("/entry-helper", ["/entry-helper", root, "/usr/bin/python3", "--restricted"], env)
