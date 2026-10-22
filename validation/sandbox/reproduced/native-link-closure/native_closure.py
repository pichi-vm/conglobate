#!/usr/bin/python3
"""Relink trusted existing probe objects with a map; inventory native inputs."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shlex
import shutil
import subprocess

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--pair', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
args = p.parse_args()
pair = args.pair.resolve()
out = args.out.resolve()
out.mkdir()
build = pair / 'worker-build/misc'


def run(command, cwd=None):
    return subprocess.check_output(command, cwd=cwd, text=True, stderr=subprocess.STDOUT, timeout=30)


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


dry = run(['make', '-n', '-W', 'fuse2fs.o', 'V=1', 'fuse2fs'], build)
(out / 'link-dry-run.txt').write_text(dry)
commands = [line for line in dry.replace('\\\n', ' ').splitlines()
            if line.strip() and not line.startswith('#')]
assert len(commands) == 1, commands
command = shlex.split(commands[0])
assert command[0] == 'gcc' and command.count('-o') == 1, command
position = command.index('-o') + 1
assert command[position] == 'fuse2fs'
command[position] = str(out / 'relinked-worker')
command += ['-Wl,-Map=' + str(out / 'link.map'), '-Wl,--trace']
(out / 'link-command.json').write_text(json.dumps(command, indent=2) + '\n')
trace = run(command, build)
(out / 'link-trace.txt').write_text(trace)
original = digest(pair / 'acl-patched')
relinked = digest(out / 'relinked-worker')
assert original == relinked, (original, relinked)

inputs = {}
for line in trace.splitlines():
    candidate = Path(line.strip())
    if not candidate.is_absolute():
        candidate = build / candidate
    if candidate.is_file():
        candidate = candidate.resolve()
        inputs[str(candidate)] = {'sha256': digest(candidate), 'bytes': candidate.stat().st_size}
assert inputs, trace
members = {}
for line in (out / 'link.map').read_text().splitlines():
    match = re.match(r'^\s*(\S+\.a)\(([^)]+)\)', line)
    if not match:
        continue
    archive = Path(match[1])
    if not archive.is_absolute():
        archive = build / archive
    archive = archive.resolve()
    member = match[2]
    key = str(archive) + '(' + member + ')'
    if key in members:
        continue
    data = subprocess.check_output(['ar', 'p', str(archive), member], timeout=5)
    members[key] = {'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)}
    inputs.setdefault(str(archive), {'sha256': digest(archive), 'bytes': archive.stat().st_size})
assert members
for name, record in inputs.items():
    if not Path(name).is_relative_to(pair):
        record['owning_package'] = run(['rpm', '-qf', name, '--qf',
                                       '%{NAME} %{VERSION}-%{RELEASE} %{LICENSE}']).strip()

# Conservative source-tree manifests include unlinked code too. They identify
# the local source snapshots, not proof that each object came from those bytes.
trees = {}
for name in ['worker-source', 'fuse-source']:
    catalog = {}
    for path in sorted((pair / name).rglob('*')):
        relative = str(path.relative_to(pair / name))
        if path.is_symlink():
            catalog[relative] = {'symlink': str(path.readlink())}
        elif path.is_file():
            catalog[relative] = {'sha256': digest(path)}
    trees[name] = catalog
configurations = {}
for base, names in [('worker-build', ['config.log', 'config.status', 'lib/config.h', 'lib/dirpaths.h', 'misc/Makefile']),
                    ('fuse-build', ['compile_commands.json', 'build.ninja', 'fuse_config.h'])]:
    for name in names:
        path = pair / base / name
        if path.is_file():
            configurations[f'{base}/{name}'] = digest(path)

tool_records = {}
for name in ['gcc', 'ld', 'ar']:
    path = Path(shutil.which(name)).resolve()
    tool_records[name] = {'version': run([name, '--version']).splitlines()[0],
                          'path': str(path), 'sha256': digest(path)}
report = {'worker_sha256': original, 'relinked_sha256': relinked,
          'byte_identical_relink': True, 'link_inputs': inputs,
          'archive_members': members, 'source_trees': trees,
          'configuration_sha256': configurations,
          'tools': tool_records,
          'limits': ['Relink binds existing objects/libraries, not their original compilation provenance.',
                     'Source-tree catalogs include unused files and do not replace preprocessing dependencies.',
                     'Dynamic loader configuration and full appliance/package maintenance remain separate.']}
(out / 'inventory.json').write_text(json.dumps(report, indent=2) + '\n')
print(f'NATIVE_RELINK_IDENTITY_PASS {len(inputs)} link files; {len(members)} archive members', flush=True)
