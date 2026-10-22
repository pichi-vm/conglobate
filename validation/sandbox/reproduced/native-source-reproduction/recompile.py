#!/usr/bin/python3
"""Recompile the retained worker's contributing C objects into a fresh directory."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shlex
import subprocess

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--inventory', type=Path, required=True)
p.add_argument('--pair', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
args = p.parse_args()
inventory = json.loads(args.inventory.read_text())
pair = args.pair.resolve()
out = args.out.resolve()
out.mkdir()


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def run(command, cwd=None):
    return subprocess.check_output(command, cwd=cwd, stderr=subprocess.STDOUT, text=True, timeout=30)


objects = {}
for name, record in inventory['link_inputs'].items():
    path = Path(name)
    if path.is_relative_to(pair) and path.suffix == '.o':
        objects[path] = record['sha256']
directories = {'libext2fs.a': 'ext2fs', 'libcom_err.a': 'et', 'libe2p.a': 'e2p'}
for name, record in inventory['archive_members'].items():
    archive, member = name[:-1].split('(', 1)
    archive = Path(archive)
    assert archive.is_relative_to(pair), archive
    objects[archive.parent / directories[archive.name] / member] = record['sha256']
records = []
headers = {}
for original, expected in sorted(objects.items()):
    assert digest(original) == expected, original
    target = out / original.relative_to(pair / 'worker-build')
    target.parent.mkdir(parents=True, exist_ok=True)
    dwarf = run(['readelf', '--debug-dump=info', str(original)])
    # Retained C objects have one compile unit. Its first name is the source;
    # later DW_AT_name entries describe types/symbols and must not be used.
    assert dwarf.count('(DW_TAG_compile_unit)') == 1, original
    source_line = next(line for line in dwarf.splitlines() if 'DW_AT_name ' in line)
    source = source_line.split('): ', 1)[1]
    source_path = Path(source)
    if not source_path.is_absolute():
        source_path = original.parent / source_path
    assert source_path.is_file(), source_path
    dry = run(['make', '-n', '-W', source, 'V=1', original.name], original.parent)
    commands = [shlex.split(line) for line in dry.replace('\\\n', ' ').splitlines()
                if line.startswith('gcc ')]
    commands = [command for command in commands if '-o' in command
                and command[command.index('-o') + 1] == original.name]
    assert len(commands) == 1, (original, dry)
    command = commands[0]
    assert '-c' in command
    command[command.index('-o') + 1] = str(target)
    dependencies = target.with_suffix('.d')
    command += ['-MD', '-MF', str(dependencies)]
    log = run(command, original.parent)
    target.with_suffix('.log').write_text(log)
    actual = digest(target)
    files = shlex.split(dependencies.read_text().replace('\\\n', ' ').split(':', 1)[1])
    dependencies_for_object = {}
    for name in files:
        path = Path(name)
        if not path.is_absolute():
            path = original.parent / path
        path = path.resolve()
        value = digest(path)
        dependencies_for_object[str(path)] = value
        assert str(path) not in headers or headers[str(path)] == value, path
        headers[str(path)] = value
    record = {'original': str(original), 'source': str(source_path), 'cwd': str(original.parent),
              'command': command, 'original_sha256': expected, 'rebuilt_sha256': actual,
              'identical': actual == expected, 'dependencies': dependencies_for_object}
    records.append(record)
    (out / 'objects.json').write_text(json.dumps(records, indent=2) + '\n')
    assert actual == expected, (original, expected, actual)
    print('NATIVE_OBJECT_REPRODUCED', original.relative_to(pair / 'worker-build'), flush=True)
for name, expected in headers.items():
    assert digest(Path(name)) == expected, f'compile input changed: {name}'
summary = {'objects': len(records), 'unique_source_and_header_inputs': len(headers),
           'all_objects_identical': True, 'inputs_stable_at_final_check': True,
           'inventory_sha256': digest(args.inventory),
           'scope': 'Existing generated configuration/source inputs and current compiler; not a from-release clean bootstrap or libfuse rebuild.'}
(out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('NATIVE_OBJECT_REPRODUCTION_PASS ' + json.dumps(summary), flush=True)
