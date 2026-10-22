#!/usr/bin/python3
"""Check indexed evidence availability and identity, not semantic sufficiency."""
import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
repo = Path(__file__).resolve().parents[2]
index = repo / 'validation/sandbox/reproduced/status-audit.json'
data = json.loads(index.read_text())
assert sorted(a['area'] for a in data['areas']) == list(range(1, 22))
errors = []
paths = set()
for area in data['areas']:
    for record in area['artifacts']:
        relative = Path(record['path'])
        assert not relative.is_absolute() and '..' not in relative.parts
        paths.add(str(relative))
        path = repo / relative
        if not path.is_file():
            errors.append({'area':area['area'], 'path':str(relative), 'reason':'missing'})
            continue
        raw = path.read_bytes()
        if len(raw) != record['bytes'] or hashlib.sha256(raw).hexdigest() != record['sha256']:
            errors.append({'area':area['area'], 'path':str(relative), 'reason':'identity mismatch'})
        exits = [line for line in raw.decode().splitlines() if line.startswith('GUEST_EXIT ')]
        if 'guest_exit_lines' in record and exits != record['guest_exit_lines']:
            errors.append({'area':area['area'], 'path':str(relative), 'reason':'exit-index mismatch'})
report = {'scope':'Availability/identity only; negative results retained; NOT semantic completion',
          'index_sha256':hashlib.sha256(index.read_bytes()).hexdigest(),
          'areas':len(data['areas']), 'unique_artifacts':len(paths), 'errors':errors}
with args.out.open('x') as output:
    output.write(json.dumps(report, indent=2)+'\n')
print(json.dumps(report, indent=2))
raise SystemExit(bool(errors))
