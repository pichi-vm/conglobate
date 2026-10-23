#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Assess resolver scaffolding/restoration across real command namespaces."""
import argparse
import json
from pathlib import Path


class ResolverEvidence:
    MARKERS = ('RESOLVER_DANGLING_INJECTION_PASS',
               'RESOLVER_DANGLING_SCAFFOLD_REMOVED_PASS',
               'RESOLVER_PARENT_REPLACED_DURING_COMMAND_PASS',
               'RESOLVER_PINNED_PARENT_CLEANUP_PASS',
               'NETWORK_RESTRICTED_IMAGE_DNS_AND_TCP_PASS',
               'NETWORK_ENVIRONMENT_RETAINED_PASS', 'DISTRO_BUILD_AND_EXPORT_PASS')

    def __init__(self, run, negative=False):
        self.run, self.negative = run, negative

    def verify(self):
        serial = (self.run / 'serial.txt').read_text()
        if self.negative:
            return self.rejected(serial)
        checks = {marker: f'\n{marker}\n' in serial for marker in self.MARKERS}
        checks['exit_zero'] = (self.run / 'status.raw').read_bytes().rstrip(b'\0') == b'DISTRO_BUILD_EXIT 0\n'
        checks['runtime_cleanup'] = 'runtime cleanup:' not in serial and 'runtime retained' not in serial
        checks['artifact_published'] = (self.run / 'artifact/inventory.cbor').is_file()
        result = {'checks': checks, 'passed': all(checks.values()),
                  'full_sandbox_qualified': False}
        (self.run / 'resolver-assessment.json').write_text(json.dumps(result, indent=2) + '\n')
        return result

    def rejected(self, serial):
        checks = {
            'replacement_reached': '\nRESOLVER_OWNED_DIRECTORY_REPLACED_PASS\n' in serial,
            'identity_mismatch_rejected': 'resolver scaffold entry was replaced' in serial,
            'build_rejected': (self.run / 'status.raw').read_bytes().rstrip(b'\0') == b'DISTRO_BUILD_EXIT 1\n',
            'output_absent': '\nFAILED_BUILD_OUTPUT_ABSENT\n' in serial,
            'environment_retained': '\nNETWORK_ENVIRONMENT_RETAINED_PASS\n' in serial,
        }
        result = {'checks': checks, 'passed': all(checks.values()),
                  'full_sandbox_qualified': False, 'cleanup_complete_claimed': False}
        (self.run / 'resolver-assessment.json').write_text(json.dumps(result, indent=2) + '\n')
        return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run', type=Path)
    parser.add_argument('--negative', action='store_true')
    args = parser.parse_args()
    result = ResolverEvidence(args.run, args.negative).verify()
    print(json.dumps(result, indent=2))
    raise SystemExit(0 if result['passed'] else 1)
