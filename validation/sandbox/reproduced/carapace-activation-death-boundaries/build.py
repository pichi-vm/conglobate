#!/usr/bin/env python3
"""Build existing activation tests from the dependency selected by Cargo.lock."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--out', required=True, type=Path)
parser.add_argument('--death-probe', action='store_true')
args = parser.parse_args()
repo = Path(__file__).resolve().parents[3]
metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--format-version', '1'], cwd=repo))
package, = [p for p in metadata['packages'] if p['name'] == 'carapace']
source = Path(package['manifest_path']).parent
out = args.out.resolve()
out.mkdir()
copy = out / 'source'
shutil.copytree(source, copy, ignore=shutil.ignore_patterns('.git', 'target'))
files = {str(p.relative_to(copy)): hashlib.sha256(p.read_bytes()).hexdigest()
         for p in sorted(copy.rglob('*')) if p.is_file()}
probe = Path(__file__).with_name('death-probe.rs')
if args.death_probe:
    target = copy / 'src/assemble.rs'
    text = target.read_text()
    edits = {
        '    Created,\n    Resumed,': '    Created,\n    Loaded,\n    Opened,\n    Resumed,',
        '                    self.control.load(id, table, true)?;':
        '                    self.control.load(id, table, true)?;\n'
        '                    #[cfg(test)]\n'
        '                    if std::env::var("CARAPACE_DEATH_STAGE").as_deref() == Ok("Loaded") {\n'
        '                        observe(ActivationStage::Loaded)?;\n'
        '                    }',
        '                    if let Err(error) = self.control.remove(id, true) {':
        '                    #[cfg(test)]\n'
        '                    if std::env::var("CARAPACE_DEATH_STAGE").as_deref() == Ok("Opened") {\n'
        '                        observe(ActivationStage::Opened)?;\n'
        '                    }\n'
        '                    if let Err(error) = self.control.remove(id, true) {',
    }
    for before, after in edits.items():
        assert text.count(before) == 1, before
        text = text.replace(before, after)
    target.write_text(text + '\n' + probe.read_text())
command = ['cargo', 'test', '--locked', '--manifest-path', str(copy / 'Cargo.toml'),
           '--target-dir', str(out / 'target'), '--lib', '--no-default-features',
           '--features', 'loop,image', '--no-run', '--message-format=json']
with (out / 'build.log').open('w') as log:
    result = subprocess.run(command, stdout=subprocess.PIPE, stderr=log, text=True)
(out / 'build-messages.jsonl').write_text(result.stdout)
result.check_returncode()
artifacts = [json.loads(line) for line in result.stdout.splitlines() if line.startswith('{')]
binaries = [Path(a['executable']) for a in artifacts if a.get('reason') == 'compiler-artifact'
            and a.get('executable') and a['target']['name'] == 'carapace' and a['profile']['test']]
binary, = binaries
shutil.copy2(binary, out / 'activation-tests')
(out / 'inputs.json').write_text(json.dumps({'package_id':package['id'], 'source':package['source'],
    'command':command, 'source_sha256':files,
    'death_probe_sha256': hashlib.sha256(probe.read_bytes()).hexdigest() if args.death_probe else None,
    'compiled_assemble_sha256':hashlib.sha256((copy/'src/assemble.rs').read_bytes()).hexdigest(),
    'binary_sha256':hashlib.sha256((out/'activation-tests').read_bytes()).hexdigest()}, indent=2)+'\n')
print(out / 'activation-tests')
