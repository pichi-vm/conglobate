#!/usr/bin/python3
"""Rebuild the retained libfuse objects/library without modifying their originals."""
import argparse
import hashlib
import json
from pathlib import Path
import shlex
import subprocess

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--pair', required=True, type=Path)
p.add_argument('--out', required=True, type=Path)
args = p.parse_args()
pair = args.pair.resolve()
build = pair / 'fuse-build'
out = args.out.resolve()
out.mkdir()


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def run(command):
    return subprocess.check_output(command, cwd=build, stderr=subprocess.STDOUT, text=True, timeout=30)


database = build / 'compile_commands.json'
records = []
headers = {}
replacements = {}
for entry in json.loads(database.read_text()):
    assert Path(entry['directory']) == build
    command = shlex.split(entry['command'])
    assert command[0] == 'cc' and '-c' in command
    original = build / entry['output']
    target = out / entry['output']
    target.parent.mkdir(parents=True, exist_ok=True)
    assert command[command.index('-o') + 1] == entry['output']
    command[command.index('-o') + 1] = str(target)
    depfile = target.with_suffix('.d')
    command[command.index('-MF') + 1] = str(depfile)
    log = run(command)
    target.with_suffix('.log').write_text(log)
    deps = {}
    for name in shlex.split(depfile.read_text().replace('\\\n', ' ').split(':', 1)[1]):
        path = Path(name)
        if not path.is_absolute():
            path = build / path
        path = path.resolve()
        value = digest(path)
        deps[str(path)] = value
        assert str(path) not in headers or headers[str(path)] == value
        headers[str(path)] = value
    expected, actual = digest(original), digest(target)
    records.append({'original': str(original), 'command': command, 'dependencies': deps,
                    'original_sha256': expected, 'rebuilt_sha256': actual,
                    'identical': expected == actual})
    (out / 'objects.json').write_text(json.dumps(records, indent=2) + '\n')
    assert expected == actual, entry['output']
    replacements[entry['output']] = str(target)
    print('FUSE_OBJECT_REPRODUCED ' + entry['output'], flush=True)

original_library = (build / 'lib/libfuse3.so.4').resolve()
relative = str(original_library.relative_to(build))
commands = run(['ninja', '-t', 'commands', relative]).splitlines()
link = shlex.split(commands[-1])
assert link[0] == 'cc' and link[link.index('-o') + 1] == relative
assert set(replacements) <= set(link)
link = [replacements.get(arg, arg) for arg in link]
link[link.index('-o') + 1] = str(out / 'rebuilt-libfuse.so')
link += ['-Wl,-Map=' + str(out / 'link.map'), '-Wl,--trace']
(out / 'link-command.json').write_text(json.dumps(link, indent=2) + '\n')
trace = run(link)
(out / 'link-trace.txt').write_text(trace)
inputs = {}
for name in trace.splitlines():
    path = Path(name.strip())
    if not path.is_absolute():
        path = build / path
    if path.is_file():
        inputs[str(path.resolve())] = digest(path)
version_script = pair / 'fuse-source/lib/fuse_versionscript'
inputs[str(version_script)] = digest(version_script)
expected, actual = digest(original_library), digest(out / 'rebuilt-libfuse.so')
assert expected == actual == digest(pair / 'libfuse3.so.4')
for name, value in headers.items():
    assert digest(Path(name)) == value, name
summary = {'objects': len(records), 'source_and_header_inputs': len(headers),
           'original_library_sha256': expected, 'rebuilt_library_sha256': actual,
           'all_objects_and_library_identical': True, 'link_input_sha256': inputs,
           'compile_database_sha256': digest(database),
           'scope': 'Current generated configuration and existing compiler environment; not a clean release/bootstrap rebuild.'}
(out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('FUSE_SOURCE_REPRODUCTION_PASS', flush=True)
