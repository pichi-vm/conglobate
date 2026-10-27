# SPDX-License-Identifier: Apache-2.0
"""Rootless construction of the trusted test guest from local host tools."""
import pathlib
import re
import shutil
import subprocess


class GuestRuntime:
    TOOLS = (
        'bash', 'cat', 'cmp', 'chmod', 'chown', 'cp', 'e2fsck', 'getcap', 'getfacl',
        'getfattr', 'git', 'grep', 'ip', 'ln', 'mkdir', 'mke2fs', 'modprobe', 'mount',
        'mv', 'newgidmap', 'newuidmap', 'nft', 'rm', 'rmdir', 'setcap',
        'setfacl', 'setfattr', 'setpriv', 'sleep', 'stat', 'touch', 'true',
        'truncate', 'umount', 'unshare',
    )
    MODULES = (
        'loop', 'ext4', 'dm-zero', 'dm-snapshot', 'dm-verity', 'veth',
        'nf_tables', 'nft_chain_nat', 'nft_masq', 'nft_ct',
    )

    def __init__(self, root, module_root, kernel_release):
        self.root = root
        self.module_root = module_root
        self.kernel_release = kernel_release

    def create(self):
        self.root.mkdir()
        self.root.chmod(0o755)
        for directory in ('usr/bin', 'usr/sbin', 'usr/lib', 'usr/lib64',
                          'etc', 'proc', 'sys', 'dev', 'tmp', 'run', 'var'):
            self.directory(self.root / directory)
        for name, target in (('bin', 'usr/bin'), ('sbin', 'usr/sbin'),
                             ('lib', 'usr/lib'), ('lib64', 'usr/lib64'), ('var/run', '../run')):
            (self.root / name).symlink_to(target)
        (self.root / 'tmp').chmod(0o1777)
        (self.root / 'etc/passwd').write_text('root:x:0:0:root:/:/bin/bash\n')
        (self.root / 'etc/group').write_text('root:x:0:\n')
        (self.root / 'etc/nsswitch.conf').write_text(
            'passwd: files\ngroup: files\nhosts: files dns\n')
        for name in self.TOOLS:
            source = shutil.which(name)
            if source is None:
                raise RuntimeError(f'missing trusted guest tool: {name}')
            self.executable(pathlib.Path(source), pathlib.Path('usr/bin') / name)
        # Formatter discovery and fallback shell capture run ldd inside the
        # trusted guest. It is a shell script, so executable() cannot inspect it
        # with ldd. Its shell and loader are already staged above.
        ldd = shutil.which('ldd')
        if ldd is None:
            raise RuntimeError('missing trusted guest tool: ldd')
        shutil.copy2(ldd, self.root / 'usr/bin/ldd')
        (self.root / 'usr/bin/sh').symlink_to('bash')
        (self.root / 'usr/sbin/modprobe').symlink_to('../bin/modprobe')
        self.modules()

    def directory(self, path):
        # Build hosts may use umask 077. Namespace helpers execute after ID
        # mapping, so the trusted guest's interpreter and library paths must
        # remain traversable by those mapped credentials.
        path.relative_to(self.root)
        path.mkdir(parents=True, exist_ok=True)
        while path != self.root:
            path.chmod(0o755)
            path = path.parent

    def executable(self, source, destination):
        target = self.root / destination
        self.directory(target.parent)
        shutil.copy2(source, target)
        target.chmod(0o755)
        # Only supplied trusted host tools are inspected. Never run ldd on
        # an untrusted recipe/image executable.
        for line in subprocess.check_output(['ldd', str(source)], text=True).splitlines():
            if 'not found' in line:
                raise RuntimeError(f'missing dependency for {source}: {line}')
            match = re.search(r'(/\S+)', line)
            if match:
                library = pathlib.Path(match.group(1))
                target = self.root / library.relative_to('/')
                self.directory(target.parent)
                shutil.copy2(library, target)

    def modules(self):
        source_tree = (self.module_root / 'lib/modules' / self.kernel_release).resolve(strict=True)
        target_tree = self.root / 'lib/modules' / self.kernel_release
        target_tree.mkdir(parents=True, exist_ok=True)
        for name in ('modules.order', 'modules.builtin', 'modules.builtin.modinfo'):
            shutil.copy2(source_tree / name, target_tree / name)
        for module in self.MODULES:
            lines = subprocess.check_output([
                'modprobe', '--config', '/dev/null', '--show-depends',
                '--dirname', str(self.module_root),
                '--set-version', self.kernel_release, module,
            ], text=True).splitlines()
            for line in lines:
                if line.startswith('insmod '):
                    source = pathlib.Path(line.split()[1]).resolve(strict=True)
                    target = target_tree / source.relative_to(source_tree)
                    self.directory(target.parent)
                    shutil.copy2(source, target)
        subprocess.run(['depmod', '-b', str(self.root), self.kernel_release], check=True)
