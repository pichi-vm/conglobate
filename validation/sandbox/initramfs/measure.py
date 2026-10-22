#!/usr/bin/env python3
"""Time repeated fully checked restricted Fedora/initramfs fixture runs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import time

parser=argparse.ArgumentParser(description=__doc__)
for option in ['seed','upgrade-rpms','protocol-worker-build','initramfs-rpms','out']:
    parser.add_argument('--'+option,required=True,type=Path)
parser.add_argument('--runs',type=int,default=2)
args=parser.parse_args()
assert 2<=args.runs<=5
out=args.out.resolve()
out.mkdir()
runner=Path(__file__).resolve().parents[1]/'policy/package.py'
records=[]
for index in range(1,args.runs+1):
    run=out/f'run-{index}'
    command=['python3',str(runner),'--restricted-bootstrap','--out',str(run)]
    for option in ['seed','upgrade-rpms','protocol-worker-build','initramfs-rpms']:
        command.extend(['--'+option,str(getattr(args,option.replace('-','_')).resolve())])
    start=time.perf_counter()
    print(f'MEASUREMENT_START {index}',flush=True)
    with (out/f'run-{index}.log').open('wb') as log:
        result=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,timeout=480)
    elapsed=time.perf_counter()-start
    record={'run':index,'command':command,'exit_code':result.returncode,'runner_wall_seconds':elapsed}
    records.append(record)
    (out/'measurements.json').write_text(json.dumps(records,indent=2)+'\n')
    assert result.returncode==0,record
    text=(run/'results.txt').read_text()
    assert [line for line in text.splitlines() if line.startswith('GUEST_EXIT')]==['GUEST_EXIT 0']
    for marker in ['VALIDATION_PASS','INITRAMFS_KERNEL_REMOUNT_PASS','PAIRED_WORKER_PACKAGE_ACL_ASSERTIONS_PASS']:
        assert marker in text,marker
    uptime=re.findall(r'\[\s*([0-9.]+)\]\s+sysrq: Resetting',text)
    assert len(uptime)==1,uptime
    record['guest_uptime_seconds']=float(uptime[0])
    record['inputs_sha256']=hashlib.sha256((run/'inputs.json').read_bytes()).hexdigest()
    record['results_sha256']=hashlib.sha256((run/'results.txt').read_bytes()).hexdigest()
    print(f'MEASUREMENT_PASS {index} runner={elapsed:.3f}s guest={uptime[0]}s',flush=True)
    (out/'measurements.json').write_text(json.dumps(records,indent=2)+'\n')
summary={'runs':args.runs,'host_kernel':os.uname().release,'machine':os.uname().machine,
         'guest_vcpus':2,'guest_ram_mib':8192,'cache_policy':'fresh guest each run; host caches not flushed',
         'runner_median_seconds':statistics.median(r['runner_wall_seconds'] for r in records),
         'guest_median_seconds':statistics.median(r['guest_uptime_seconds'] for r in records),
         'scope':'Whole validated fixture including control probes and teardown, not isolated DNF/dracut time or comparative backend speed.'}
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary),flush=True)
