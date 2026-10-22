#!/usr/bin/env python3
"""Run the current guest runner's metadata checks on an already completed log."""
import argparse
from types import SimpleNamespace
import ast
import json
from pathlib import Path
import sys

here = Path(__file__).resolve().parents[1]
runner = here / 'guest.py'
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('log', type=Path)
parser.add_argument('--expect-birthtime', action='store_true')
options = parser.parse_args()
text = options.log.read_text()
assert [l for l in text.splitlines() if l.startswith('GUEST_EXIT ')] == ['GUEST_EXIT 0']
tree = ast.parse(runner.read_text())
blocks = [n for n in ast.walk(tree) if isinstance(n, ast.If)
          and ast.unparse(n.test) == "args.case == 'metadata'"]
assert len(blocks) == 1
exec(compile(ast.Module(body=blocks[0].body, type_ignores=[]), str(runner), 'exec'),
     {'result': text, 'HERE': here, 'json': json, 'args': SimpleNamespace(expect_birthtime=options.expect_birthtime)})
print('SAVED_GUEST_METADATA_AND_ENOSPC_VALIDATION_PASS')
