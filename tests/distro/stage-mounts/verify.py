#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Verify stage filesystem behavior and output publication independently."""
import argparse
import json
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from artifact import build_manifest


class StageMountEvidence:
    def __init__(self, run, mode):
        self.run, self.mode = run, mode

    def verify(self):
        serial = (self.run / 'serial.txt').read_text()
        status = (self.run / 'status.raw').read_bytes().rstrip(b'\0')
        checks = {'runtime_closed': 'runtime cleanup:' not in serial and
                  'runtime retained' not in serial and 'retained until VM disposal' not in serial}
        if self.mode == 'success':
            for marker in ['STAGE_CREATION_MOUNTS_PASS', 'STAGE_NESTED_COPY_AND_RENAME_PASS',
                           'STAGE_EXTENSION_MOUNTS_PASS', 'STAGE_ESP_PERSISTS_TMPFS_DISAPPEARS_PASS',
                           'STAGE_MOUNTS_NOT_INHERITED_PASS', 'DISTRO_BUILD_AND_EXPORT_PASS']:
                checks[marker] = f'\n{marker}\n' in serial
            checks['exit_zero'] = status == b'DISTRO_BUILD_EXIT 0\n'
            manifest = build_manifest(self.run)
            checks['one_layer_per_stage'] = [layer['mediaType'] for layer in manifest['layers']] == [
                'application/vnd.pichi.scute.v1'] * 4
        else:
            marker, diagnostic = ('STAGE_MOUNTS_EXPECTED_FAILURE', 'command exited with exit status: 7') if self.mode == 'nonzero' else (
                'STAGE_MOUNTS_TIMEOUT_STARTED', 'deadline expired')
            checks['failure_reached'] = f'\n{marker}\n' in serial
            checks['expected_failure'] = diagnostic in serial
            checks['no_continuation'] = '\nSTAGE_UNEXPECTED_CONTINUATION\n' not in serial
            checks['build_failed'] = status == b'DISTRO_BUILD_EXIT 1\n'
            checks['output_absent'] = '\nFAILED_BUILD_OUTPUT_ABSENT\n' in serial
            checks['command_cleanup_complete'] = 'cleanup: complete' in serial
        result = {'mode': self.mode, 'checks': checks, 'passed': all(checks.values())}
        (self.run / 'stage-mount-assessment.json').write_text(json.dumps(result, indent=2) + '\n')
        return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run', type=Path)
    parser.add_argument('mode', choices=['success', 'nonzero', 'timeout'])
    args = parser.parse_args()
    result = StageMountEvidence(args.run, args.mode).verify()
    print(json.dumps(result, indent=2))
    raise SystemExit(0 if result['passed'] else 1)
