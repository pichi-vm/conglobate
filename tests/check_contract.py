#!/usr/bin/env python3
# SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
# SPDX-License-Identifier: Apache-2.0
"""Check the library/executable dependency boundary using Cargo metadata."""
import json
from pathlib import Path
import subprocess


def main():
    repository = Path(__file__).resolve().parents[1]
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--locked', '--no-deps', '--format-version=1'],
        cwd=repository,
    ))
    package = next(p for p in metadata['packages'] if p['name'] == 'conglobate')
    if package['features'].get('default', []):
        raise SystemExit('the contract library must not enable default features')
    normal = {d['name'] for d in package['dependencies']
              if d['kind'] is None and not d['optional']}
    if normal != {'serde', 'hex', 'thiserror'}:
        raise SystemExit(f'unexpected library dependencies: {sorted(normal)}')
    binary = next(t for t in package['targets']
                  if t['name'] == 'conglobate' and 'bin' in t['kind'])
    if binary.get('required-features') != ['cli']:
        raise SystemExit('the executable must require the cli feature')
    subprocess.run(['cargo', 'check', '--locked', '--no-default-features', '--lib'],
                   cwd=repository, check=True)
    print('contract boundary passed: default library is lightweight; binary requires cli')


if __name__ == '__main__':
    main()
