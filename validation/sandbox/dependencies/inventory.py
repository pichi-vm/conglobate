#!/usr/bin/python3
"""Inventory actual prototype dependencies; not a license-compliance verdict."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--worker', type=Path, required=True)
parser.add_argument('--e2fs-source', type=Path, required=True)
parser.add_argument('--fuse-source', type=Path, required=True)
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
args.out.mkdir()
root = Path(__file__).resolve().parents[1]
helper = root / 'entry/helper'

def run(*command):
    return subprocess.check_output(command, text=True).strip()

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

metadata = json.loads(run('cargo', 'metadata', '--locked', '--offline', '--format-version', '1',
                          '--manifest-path', str(helper / 'Cargo.toml')))
nodes = {node['id']: node for node in metadata['resolve']['nodes']}
packages = []
for package in metadata['packages']:
    directory = Path(package['manifest_path']).parent
    licenses = {}
    for pattern in ('LICENSE*', 'COPYING*', 'NOTICE*'):
        for path in directory.glob(pattern):
            if path.is_file():
                licenses[path.name] = digest(path)
    packages.append({'name': package['name'], 'version': package['version'],
                     'source': package['source'], 'license': package['license'],
                     'license_file': package['license_file'], 'license_hashes': licenses,
                     'manifest_sha256': digest(Path(package['manifest_path'])),
                     'features': nodes[package['id']]['features'],
                     'dependencies': nodes[package['id']]['dependencies']})
ldd = run('ldd', str(args.worker.resolve()))
if 'not found' in ldd:
    raise RuntimeError(ldd)
libraries = []
for name in sorted(set(re.findall(r'(/[^\s()]+)', ldd))):
    path = Path(name).resolve()
    libraries.append({'path': str(path), 'sha256': digest(path),
                      'package': run('rpm', '-qf', str(path), '--qf',
                                     '%{NAME} %{VERSION}-%{RELEASE} %{LICENSE}')})
notices = {}
for label, directory, names in (
    ('e2fsprogs', args.e2fs_source, ('NOTICE', 'misc/fuse2fs.c')),
    ('libfuse', args.fuse_source, ('LICENSE', 'LGPL2.txt', 'GPL2.txt')),
):
    notices[label] = {name: digest(directory / name) for name in names}
report = {'scope': 'entry helper Cargo graph and supplied trusted native worker runtime',
          'cargo': run('cargo', '--version'), 'rustc': run('rustc', '--version'),
          'lock_sha256': digest(helper / 'Cargo.lock'), 'packages': packages,
          'worker': {'path': str(args.worker.resolve()), 'sha256': digest(args.worker)},
          'libraries': libraries, 'upstream_notice_hashes': notices,
          'limits': ['Declared licenses and local notice identities only; no distribution approval.',
                     'Native worker source/patch-to-binary binding uses separate build records.',
                     'Not the full appliance/package dependency inventory or a soundness audit.']}
(args.out / 'inventory.json').write_text(json.dumps(report, indent=2) + '\n')
(args.out / 'ldd.txt').write_text(ldd + '\n')
assert all(package['license'] or package['license_file'] for package in packages)
print(f'DEPENDENCY_INVENTORY_PASS {len(packages)} Cargo packages; {len(libraries)} native libraries/loader')
