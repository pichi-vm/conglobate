#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Ordinary-user, loopback-only endpoint outside the disposable build VM."""
import argparse
import json
from pathlib import Path
import secrets
import socket
import subprocess
import sys


class ExternalService:
    def __init__(self, output):
        self.output = output

    def __enter__(self):
        self.log = (self.output / 'external-service.log').open('wb')
        self.process = subprocess.Popen(
            [sys.executable, __file__, '--serve', str(self.output)],
            stdout=subprocess.PIPE, stderr=self.log, text=True)
        try:
            self.endpoint = json.loads(self.process.stdout.readline())
            return self
        except BaseException:
            self.__exit__(None, None, None)
            raise

    def __exit__(self, *_):
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=5)
        self.process.stdout.close()
        self.log.close()

    def context_input(self):
        return f"{self.endpoint['port']} {self.endpoint['token']}\n"

    def finish(self):
        if self.process.wait(timeout=5) != 0:
            raise RuntimeError('external fixture failed; inspect external-service.log')
        report = json.loads((self.output / 'external-service.json').read_text())
        if not report['passed']:
            raise RuntimeError('external fixture exchange failed')

    @staticmethod
    def serve(output):
        token = secrets.token_hex(16)
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
            listener.bind(('127.0.0.1', 0))
            listener.listen(1)
            listener.settimeout(300)
            endpoint = {'port': listener.getsockname()[1], 'token': token}
            print(json.dumps(endpoint), flush=True)
            connection, peer = listener.accept()
            with connection:
                connection.settimeout(5)
                request = bytearray()
                while not request.endswith(b'\n') and len(request) < 128:
                    chunk = connection.recv(128 - len(request))
                    if not chunk:
                        break
                    request.extend(chunk)
                passed = request == (token + '\n').encode()
                if passed:
                    connection.sendall(b'external-reply\n')
            report = {'passed': passed, 'peer': peer, 'endpoint': endpoint,
                      'bound_address': '127.0.0.1', 'privileged_host_setup': False}
            (output / 'external-service.json').write_text(json.dumps(report, indent=2) + '\n')
            if not passed:
                raise RuntimeError('unexpected external fixture request')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--serve', type=Path, required=True)
    ExternalService.serve(parser.parse_args().serve)
