# SPDX-License-Identifier: Apache-2.0
"""Trusted disposable-VM environment fixture; never loads policy on the host."""
from pathlib import Path
import os
import shutil
import shlex
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
                'with the BPF target, a C compiler and libbpf development flags '
                '(pkg-config or CONGLOBATE_TEST_BPF_CFLAGS/LIBS). '
                'This is a test-environment prerequisite, '
                'not a conglobate binary dependency. Compiler diagnostics and '
                'the original failure are retained above.'
            ) from error

    @staticmethod
    def flags(kind):
        override = os.environ.get('CONGLOBATE_TEST_BPF_' + kind.upper())
        if override is not None:
            return shlex.split(override)
        try:
            return shlex.split(subprocess.check_output(
                ['pkg-config', '--' + kind, 'libbpf'], text=True))
        except (OSError, subprocess.CalledProcessError) as error:
            raise RuntimeError('libbpf development files or explicit '
                               'CONGLOBATE_TEST_BPF_CFLAGS/LIBS are required') from error

    def includes(self):
        flags = self.flags('cflags')
        multiarch = subprocess.check_output(['cc', '-print-multiarch'], text=True).strip()
        if multiarch and '/' not in multiarch:
            directory = Path('/usr/include') / multiarch
            if directory.is_dir():
                flags.append('-I' + str(directory))
        return flags

    def stage(self):
        source = self.repository / 'tests/environment'
        retained = self.output / 'label-environment'
        retained.mkdir()
        for name in ['labels.bpf.c', 'loader.c']:
            shutil.copy2(source / name, retained / name)
        env = os.environ.copy()
        compiler_tmp = self.output / 'build-tmp'
        compiler_tmp.mkdir(exist_ok=True)
        env['TMPDIR'] = str(compiler_tmp)
        headers = self.includes()
        libraries = self.flags('libs')
        self.compile(['clang', '-O2', '-g', '-Wall', '-Werror', '-target', 'bpf',
            *headers, '-c', str(retained / 'labels.bpf.c'), '-o',
            str(self.runtime.root / 'labels.bpf.o')], env=env)
        loader = retained / 'loader'
        self.compile(['cc', '-O2', '-Wall', '-Wextra', '-Werror', *headers,
            str(retained / 'loader.c'), *libraries, '-o', str(loader)],
            env=env)
        self.runtime.executable(loader, Path('label-loader'))

    def stage_second_filesystem_fault(self):
        source = self.repository / 'tests/distro/labeling-fault'
        retained = self.output / 'label-environment/fault'
        shutil.copytree(source, retained)
        env = os.environ.copy()
        compiler_tmp = self.output / 'build-tmp'
        compiler_tmp.mkdir(exist_ok=True)
        env['TMPDIR'] = str(compiler_tmp)
        headers = self.includes()
        libraries = self.flags('libs')
        self.compile(['clang', '-O2', '-g', '-Wall', '-Werror', '-target', 'bpf',
            *headers, '-c', str(retained / 'fault.bpf.c'), '-o',
            str(self.runtime.root / 'label-fault.bpf.o')], env=env)
        loader = retained / 'loader'
        self.compile(['cc', '-O2', '-Wall', '-Wextra', '-Werror', *headers,
            str(retained / 'loader.c'), *libraries, '-o', str(loader)],
            env=env)
        self.runtime.executable(loader, Path('label-fault'))

    @staticmethod
    def startup():
        return '''touch /run/global-label-fixture
mount -t bpf bpf /sys/fs/bpf
/label-loader
'''
