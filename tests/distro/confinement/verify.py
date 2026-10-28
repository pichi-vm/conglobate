#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Assess the benign CLI confinement observations; no full sandbox claim."""
import argparse
import json
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from artifact import build_manifest


class ConfinementEvidence:
    MARKERS = ('COMMAND_KEY_SENTINEL_SEEDED_PASS', 'COMMAND_KEY_SENTINEL_RETAINED_PASS', 'COMMAND_DESCRIPTOR_BOUNDARY_PASS', 'COMMAND_CREDENTIAL_BOUNDARY_PASS',
               'COMMAND_RUNTIME_BOUNDARY_PASS', 'COMMAND_RUNTIME_RECREATED_PASS',
               'COMMAND_PROC_VISIBILITY_PASS', 'COMMAND_SYSCALL_CONTROL_PASS', 'COMMAND_SYSCALL_DENIAL_PASS', 'COMMAND_ALLOWED_PROCESS_AND_SOCKET_PASS',
               'COMMAND_EXITS_WITH_LIVE_DESCENDANT_PASS', 'COMMAND_AFTER_DESCENDANT_CLEANUP_PASS',
               'DISTRO_BUILD_AND_EXPORT_PASS')

    def __init__(self, run):
        self.run = run

    def verify(self):
        serial = (self.run / 'serial.txt').read_text()
        checks = {marker: f'\n{marker}\n' in serial for marker in self.MARKERS}
        checks['exit_zero'] = (self.run / 'status.raw').read_bytes().rstrip(b'\0') == b'DISTRO_BUILD_EXIT 0\n'
        checks['no_live_extra_descriptor'] = '\nUNEXPECTED_LIVE_DESCRIPTOR ' not in serial
        manifest = build_manifest(self.run)
        checks['build_annotations_verified'] = bool(manifest)
        report = {'checks': checks, 'passed': all(checks.values()),
                  'full_sandbox_qualified': False, 'platforms': ['native amd64 ordinary VM']}
        (self.run / 'confinement-assessment.json').write_text(json.dumps(report, indent=2) + '\n')
        return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run', type=Path)
    args = parser.parse_args()
    result = ConfinementEvidence(args.run).verify()
    print(json.dumps(result, indent=2))
    raise SystemExit(0 if result['passed'] else 1)
