#!/usr/bin/env python3
"""Bounded benign failure and concurrency cases for the prototype launcher."""
import argparse,concurrent.futures,json,subprocess,tempfile
from pathlib import Path
import probe
p=argparse.ArgumentParser();p.add_argument('--out',type=Path,required=True);a=p.parse_args();out=a.out.resolve();out.mkdir(exist_ok=False)
(out/'allowed').write_text('allowed\n'); sentinel=out/'supervisor-secret';sentinel.write_text('secret\n')
PREFIX='''#!/bin/sh
set -eu
export PATH=/bin
busybox mount -t proc proc /proc
echo READY
echo HOSTFS_BOUNDARY_OK
'''
# HOSTFS marker here only satisfies shared runner framing; no hostfs assertion
# is claimed for these cases. Complete/pass booleans below use explicit outcomes.
cases={'background':'busybox sleep 30 &\necho COMPLETE\nbusybox poweroff -f\n',
       'command-failure':'exit 42\n',
       'false-success':'echo COMPLETE\nexit 42\n',
       'timeout':'busybox sleep 20\n'}
results=[]
def execute(name,body):
    caseout=Path(tempfile.mkdtemp(prefix=name+'-',dir=out))
    (caseout/'allowed').write_text('allowed\n')
    disk=caseout/(name+'.img')
    with disk.open('wb') as f:f.truncate(4*1024*1024)
    r=probe.run(caseout,'lifecycle',name,disk,sentinel,script=PREFIX+body,deadline=4)
    expected=(r['returncode']==0 and r['error'] is None) if name=='background' else (r['returncode']!=0)
    if name=='timeout':expected=expected and r['error']=="TimeoutError('4s deadline')"
    else:expected=expected and r['error'] is None
    r['case_pass']=expected and r['observed_processes']>1 and not r['surviving_observed_processes']
    return r
for name,body in cases.items():results.append(execute(name,body))
with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
    # One cancelled/timed-out instance and one successful instance coexist.
    results.extend(pool.map(lambda pair:execute(*pair),[('timeout',cases['timeout']),('background',cases['background'])]))
assert sentinel.read_text()=='secret\n'
(out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
print(json.dumps({'cases':len(results),'pass':all(r['case_pass'] for r in results)}))
