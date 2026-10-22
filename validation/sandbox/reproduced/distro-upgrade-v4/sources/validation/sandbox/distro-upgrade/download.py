#!/usr/bin/env python3
"""Fetch explicit public Fedora release URLs into a new bounded fixture store."""
import argparse,hashlib,json,subprocess,urllib.request,urllib.parse
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--inputs',type=Path,required=True);a=p.parse_args()
root=a.inputs.resolve(); jobs=[]
for release in (43,44):
    target=root/f'rpms{release}';target.mkdir()
    urls=(root/f'f{release}-urls.txt').read_text().splitlines()
    assert 0<len(urls)<=160 and len(set(urls))==len(urls)
    prefix=f'https://dl.fedoraproject.org/pub/fedora/linux/releases/{release}/Everything/x86_64/os/Packages/'
    for url in urls:
        assert url.startswith(prefix)
        name=Path(urllib.parse.urlsplit(url).path).name
        assert name.endswith(('.x86_64.rpm','.noarch.rpm'))
        jobs.append((release,url,target/name))
def fetch(job):
    release,url,path=job;h=hashlib.sha256();size=0
    with urllib.request.urlopen(url,timeout=45) as source,path.open('xb') as dest:
        assert source.geturl().startswith('https://')
        while chunk:=source.read(1024*1024):
            size+=len(chunk);assert size<=32*1024*1024
            h.update(chunk);dest.write(chunk)
    return {'release':release,'url':url,'path':str(path.relative_to(root)),'bytes':size,'sha256':h.hexdigest()}
with ThreadPoolExecutor(max_workers=4) as pool: records=list(pool.map(fetch,jobs))
assert sum(r['bytes'] for r in records)<=512*1024*1024
(root/'rpms.json').write_text(json.dumps(records,indent=2)+'\n')
for release in (43,44):
    with (root/f'createrepo{release}.log').open('wb') as log:
        subprocess.run(['createrepo_c',str(root/f'rpms{release}')],stdout=log,stderr=subprocess.STDOUT,timeout=60,check=True)
print('DISTRO_FIXTURE_DOWNLOAD_PASS',len(records),sum(r['bytes'] for r in records))
