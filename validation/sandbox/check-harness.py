#!/usr/bin/python3
"""Regression checks for evidence acceptance, without launching a guest."""
import importlib.util
from pathlib import Path

spec = importlib.util.spec_from_file_location("guest", Path(__file__).with_name("guest.py"))
guest = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guest)
guest.validate_guest_exit("fixture passed\r\nGUEST_EXIT 0\r\n[1.0] sysrq: Resetting\r\n")
for output in (
    "", "GUEST_EXIT 1\n", "GUEST_EXIT 01\n", "GUEST_EXIT 0 trailing\n",
    "prefix GUEST_EXIT 0\n", "GUEST_EXIT 0\nGUEST_EXIT 1\n",
    "GUEST_EXIT 0\nGUEST_EXIT 0\n",
):
    try:
        guest.validate_guest_exit(output)
    except RuntimeError:
        pass
    else:
        raise AssertionError(f"invalid exit record accepted: {output!r}")
sources = guest.capture_sources("pressure")
for name in ("src/build.rs", "Cargo.lock", "validation/sandbox/guest.py",
             "validation/sandbox/pressure/init",
             "validation/sandbox/publication/helper/src/chain_publication.rs",
             "validation/sandbox/entry/helper/Cargo.lock"):
    assert name in sources and sources[name], name
assert not any("target" in Path(name).parts for name in sources)
print("HARNESS_EXIT_AND_SOURCE_CATALOG_PASS")
