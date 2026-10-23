# SPDX-License-Identifier: Apache-2.0
"""Trusted disposable-VM environment fixture; never loads policy on the host."""
from pathlib import Path
import os
import shutil
import subprocess


class LabelEnvironment:
    def __init__(self, repository, output, runtime):
        self.repository = repository
        self.output = output
        self.runtime = runtime

    def stage(self):
        source = self.repository / 'validation/global-label-policy'
        retained = self.output / 'label-environment'
        retained.mkdir()
        for name in ['labels.bpf.c', 'loader.c']:
            shutil.copy2(source / name, retained / name)
        env = os.environ.copy()
        env['TMPDIR'] = '/var/tmp/conglobate-rewrite-tmp'
        headers = '-I/var/tmp/conglobate-cap-review-bpf/sysroot/usr/include'
        subprocess.run(['clang', '-O2', '-g', '-Wall', '-Werror', '-target', 'bpf',
            headers, '-c', str(retained / 'labels.bpf.c'), '-o',
            str(self.runtime.root / 'labels.bpf.o')], env=env, check=True)
        loader = retained / 'loader'
        subprocess.run(['cc', '-O2', '-Wall', '-Wextra', '-Werror', headers,
            str(retained / 'loader.c'), '/usr/lib64/libbpf.so.1', '-o', str(loader)],
            env=env, check=True)
        self.runtime.executable(loader, Path('label-loader'))

    @staticmethod
    def startup():
        return '''touch /run/global-label-fixture
mount -t bpf bpf /sys/fs/bpf
/label-loader
'''
