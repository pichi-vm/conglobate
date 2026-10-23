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

    def compile(self, command, env):
        try:
            subprocess.run(command, env=env, check=True)
        except (OSError, subprocess.CalledProcessError) as error:
            raise RuntimeError(
                'Disposable-VM labeling fixture compilation failed. Requires clang '
                'with the BPF target, a C compiler, libbpf headers under '
                '/var/tmp/conglobate-cap-review-bpf/sysroot/usr/include and '
                '/usr/lib64/libbpf.so.1. This is a test-environment prerequisite, '
                'not a conglobate binary dependency. Compiler diagnostics and '
                'the original failure are retained above.'
            ) from error

    def stage(self):
        source = self.repository / 'validation/global-label-policy'
        retained = self.output / 'label-environment'
        retained.mkdir()
        for name in ['labels.bpf.c', 'loader.c']:
            shutil.copy2(source / name, retained / name)
        env = os.environ.copy()
        env['TMPDIR'] = '/var/tmp/conglobate-rewrite-tmp'
        headers = '-I/var/tmp/conglobate-cap-review-bpf/sysroot/usr/include'
        self.compile(['clang', '-O2', '-g', '-Wall', '-Werror', '-target', 'bpf',
            headers, '-c', str(retained / 'labels.bpf.c'), '-o',
            str(self.runtime.root / 'labels.bpf.o')], env=env)
        loader = retained / 'loader'
        self.compile(['cc', '-O2', '-Wall', '-Wextra', '-Werror', headers,
            str(retained / 'loader.c'), '/usr/lib64/libbpf.so.1', '-o', str(loader)],
            env=env)
        self.runtime.executable(loader, Path('label-loader'))

    def stage_second_filesystem_fault(self):
        source = self.repository / 'tests/distro/labeling-fault'
        retained = self.output / 'label-environment/fault'
        shutil.copytree(source, retained)
        env = os.environ.copy()
        env['TMPDIR'] = '/var/tmp/conglobate-rewrite-tmp'
        headers = '-I/var/tmp/conglobate-cap-review-bpf/sysroot/usr/include'
        self.compile(['clang', '-O2', '-g', '-Wall', '-Werror', '-target', 'bpf',
            headers, '-c', str(retained / 'fault.bpf.c'), '-o',
            str(self.runtime.root / 'label-fault.bpf.o')], env=env)
        loader = retained / 'loader'
        self.compile(['cc', '-O2', '-Wall', '-Wextra', '-Werror', headers,
            str(retained / 'loader.c'), '/usr/lib64/libbpf.so.1', '-o', str(loader)],
            env=env)
        self.runtime.executable(loader, Path('label-fault'))

    @staticmethod
    def startup():
        return '''touch /run/global-label-fixture
mount -t bpf bpf /sys/fs/bpf
/label-loader
'''
