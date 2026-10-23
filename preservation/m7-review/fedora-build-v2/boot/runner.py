#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Optional pichi/dillo boot interoperability; no conglobate runtime dependency."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import selectors
import shutil
import signal
import subprocess
import time


class BootRun:
    def __init__(self, arguments):
        self.arguments = arguments
        self.output = arguments.output.resolve()

    def run(self):
        self.output.mkdir()
        env = os.environ.copy()
        env['XDG_DATA_HOME'] = str(self.output / 'data')
        env['PICHI_DILLO'] = str(Path(shutil.which('dillo')).resolve())
        commands = [['pichi', 'load', str(self.arguments.artifact.resolve()),
                     '--tag', 'conglobate-qualification:local'],
                    ['pichi', 'run', '--memory', '2048', 'conglobate-qualification:local']]
        (self.output / 'commands.json').write_text(json.dumps(commands, indent=2) + '\n')
        tools = {}
        for name in ['pichi', 'dillo']:
            path = Path(shutil.which(name)).resolve()
            with path.open('rb') as stream:
                tools[name] = {'path': str(path), 'sha256': hashlib.file_digest(stream, 'sha256').hexdigest()}
        (self.output / 'tools.json').write_text(json.dumps(tools, indent=2) + '\n')
        with (self.output / 'load.log').open('wb') as log:
            subprocess.run(commands[0], env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        guest = subprocess.Popen(commands[1], env=env, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, start_new_session=True)
        passed = False
        reason = 'marker not observed'
        try:
            deadline = time.monotonic() + self.arguments.timeout
            observed = bytearray()
            with selectors.DefaultSelector() as events, (self.output / 'console.log').open('wb') as log:
                events.register(guest.stdout, selectors.EVENT_READ)
                while time.monotonic() < deadline:
                    if not events.select(max(0, deadline - time.monotonic())):
                        reason = 'boot deadline expired'
                        break
                    chunk = os.read(guest.stdout.fileno(), 65536)
                    if not chunk:
                        reason = 'launcher exited before marker'
                        break
                    log.write(chunk)
                    log.flush()
                    observed.extend(chunk)
                    if b'Kernel panic' in observed or b'ERROR corium:' in observed:
                        reason = 'guest reported boot failure'
                        break
                    if self.arguments.expect.encode() in observed:
                        passed = True
                        reason = 'expected guest marker observed; harness terminated VM'
                        break
        finally:
            if guest.poll() is None:
                os.killpg(guest.pid, signal.SIGTERM)
                try:
                    guest.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(guest.pid, signal.SIGKILL)
                    guest.wait()
            guest.stdout.close()
            (self.output / 'verdict.json').write_text(json.dumps({
                'marker_observed': passed, 'expected_marker': self.arguments.expect,
                'reason': reason, 'confidential_platform_qualified': False,
            }, indent=2) + '\n')
        if not passed:
            raise RuntimeError(reason)
        print('BOOT MARKER PASS', self.arguments.expect)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('artifact', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--expect', default='CONGLOBATE_FEDORA_ENFORCING_BOOT_PASS')
    parser.add_argument('--timeout', type=int, default=180)
    BootRun(parser.parse_args()).run()
