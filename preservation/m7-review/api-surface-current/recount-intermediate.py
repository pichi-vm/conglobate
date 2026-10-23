# SPDX-License-Identifier: Apache-2.0
"""Lexical audit aid, never a dead-code or inferred-type-usage oracle."""
import hashlib
import json
from pathlib import Path
import re
import subprocess


class SurfaceAudit:
    def run(self):
        original = json.loads(Path('preservation/m7-review/partial-agent-findings.json').read_text())
        findings = {}
        pending = [original]
        while pending:
            value = pending.pop()
            if isinstance(value, dict):
                if value.get('id') in ('api-surface-004', 'api-surface-005'):
                    findings[value['id']] = value
                pending.extend(value.values())
            elif isinstance(value, list):
                pending.extend(value)
        names = sorted({name.strip() for value in findings.values()
                        for group in re.findall(r'\{([^}]+)\}', value['suggested_fix'])
                        for name in group.split(',')})
        paths = subprocess.check_output(
            ['rg', '--files', 'src', 'tests', 'validation', '-g', '*.rs'], text=True).splitlines()
        paths = [path for path in paths if not path.startswith('validation/') or
                 ('/src/' in path and '/evidence/' not in path and '/target/' not in path)]
        sources = {path: Path(path).read_text() for path in paths}
        entries = []
        for name in names:
            pattern = re.compile(r'\b' + re.escape(name) + r'\b')
            hits = [{'path': path, 'line': number, 'text': line.strip()}
                    for path, text in sources.items()
                    for number, line in enumerate(text.splitlines(), 1) if pattern.search(line)]
            entries.append({'name': name, 'mentions': hits,
                            'external_files': sorted({h['path'] for h in hits if not h['path'].startswith('src/')}),
                            'classification': 'lexical only; zero external spelling does not imply unused'})
        result = {'scope': 'Rust sources in src, tests and validation/*/src; includes doc comments. Excludes legacy/reference, historical evidence snapshots and compiled dependency caches. Same-spelled names are not resolved by module. Inferred uses require signature review.',
                  'source_sha256': {path: hashlib.sha256(text.encode()).hexdigest() for path, text in sources.items()},
                  'candidate_count': len(entries), 'entries': entries}
        output = Path(__file__).with_name('recount.json')
        output.write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps({'files': len(sources), 'candidates': len(entries),
                          'with_external_spelling': sum(bool(e['external_files']) for e in entries),
                          'no_spelling_anywhere': [e['name'] for e in entries if not e['mentions']]}))


if __name__ == '__main__':
    SurfaceAudit().run()
