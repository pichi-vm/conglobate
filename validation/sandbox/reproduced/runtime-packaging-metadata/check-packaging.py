#!/usr/bin/env python3
"""Bounded newc packaging controls on owned text fixtures; no guest execution."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile


def unpack(raw):
    entries = {}
    offset = 0
    while True:
        assert raw[offset:offset + 6] == b'070701'
        fields = [int(raw[offset + 6 + n * 8:offset + 14 + n * 8], 16)
                  for n in range(13)]
        inode, mode, uid, gid, nlink, mtime, size, major, minor, rmajor, rminor, namesize, checksum = fields
        offset += 110
        name = raw[offset:offset + namesize - 1].decode()
        assert raw[offset + namesize - 1] == 0
        offset = (offset + namesize + 3) & ~3
        payload = raw[offset:offset + size]
        assert len(payload) == size
        offset = (offset + size + 3) & ~3
        if name == 'TRAILER!!!':
            assert not any(raw[offset:])
            break
        assert name not in entries
        entries[name] = dict(inode=inode, mode=mode, uid=uid, gid=gid,
                             nlink=nlink, size=size, major=major, minor=minor,
                             payload_hex=payload.hex())
    return entries


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='conglobate-packaging-') as tmp:
        root = Path(tmp)
        directory = root / 'directory'
        directory.mkdir(mode=0o755)
        directory.chmod(0o755)
        data = directory / 'owned'
        data.write_bytes(b'owned text fixture\n')
        data.chmod(0o640)
        os.link(data, directory / 'hardlink')
        (root / 'symlink').symlink_to('directory/owned')
        paths = sorted(str(p.relative_to(root)) for p in root.rglob('*'))

        def archive():
            result = subprocess.run(
                ['/usr/bin/cpio', '--null', '-o', '--format=newc', '--owner=0:0'],
                input=b'\0'.join(p.encode() for p in paths) + b'\0', cwd=root,
                capture_output=True, check=True, timeout=10)
            return unpack(result.stdout)

        baseline = archive()
        assert set(baseline) == set(paths)
        assert all(e['uid'] == e['gid'] == 0 for e in baseline.values())
        assert baseline['directory']['mode'] & 0o7777 == 0o755
        assert baseline['directory/owned']['mode'] & 0o7777 == 0o640
        a, b = (baseline['directory/' + n] for n in ('owned', 'hardlink'))
        assert (a['inode'], a['major'], a['minor']) == (b['inode'], b['major'], b['minor'])
        assert a['nlink'] == b['nlink'] == 2
        assert sorted((a['payload_hex'], b['payload_hex'])) == ['', data.read_bytes().hex()]
        assert bytes.fromhex(baseline['symlink']['payload_hex']) == b'directory/owned'
        content_digest = hashlib.sha256(data.read_bytes()).hexdigest()
        data.chmod(0o600)
        directory.chmod(0o700)
        changed = archive()
        assert hashlib.sha256(data.read_bytes()).hexdigest() == content_digest
        assert changed['directory']['mode'] & 0o7777 == 0o700
        assert changed['directory/owned']['mode'] & 0o7777 == 0o600
        os.setxattr(data, 'user.conglobate-fixture', b'owned metadata')
        with_xattr = archive()
        assert os.getxattr(data, 'user.conglobate-fixture') == b'owned metadata'
        # Parsed fields omit timestamps; ctime changes must not be confused
        # with serialization of the xattr name/value. Full archive parsed above.
        assert with_xattr == changed
        report = {
            'scope': 'Owned text fixture; exact diagnostic guest cpio invocation, not runtime immutability',
            'checks': ['normalized archive owners', 'file and directory modes recorded',
                       'hardlink relationship and data recorded', 'symlink target recorded',
                       'same file hash permits different executable metadata',
                       'user xattr absent from parsed archive'],
            'baseline': baseline, 'changed_modes': changed,
            'with_user_xattr': with_xattr,
            'source_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            'cpio_sha256': hashlib.sha256(Path('/usr/bin/cpio').read_bytes()).hexdigest(),
            'cpio_version': subprocess.check_output(['/usr/bin/cpio', '--version'], text=True),
        }
        with args.out.open('x') as output:
            json.dump(report, output, indent=2)
            output.write('\n')
        print('RUNTIME_PACKAGING_METADATA_CONTROLS_PASS', len(report['checks']))


if __name__ == '__main__':
    main()
