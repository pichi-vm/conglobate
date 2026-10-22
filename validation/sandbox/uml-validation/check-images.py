#!/usr/bin/env python3
"""Run read-only filesystem checkers behind a separate outer confinement."""
import argparse,json,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('images',type=Path);a=p.parse_args()
checks={'ext4':['e2fsck','-fn','/image'],'xfs':['xfs_repair','-n','/image'],'btrfs':['btrfs','check','--readonly','/image']}
results=[]
for fs,tool in checks.items():
    cmd=['bwrap','--unshare-all','--die-with-parent','--new-session','--cap-drop','ALL','--clearenv','--setenv','PATH','/usr/sbin:/usr/bin',
         '--ro-bind','/usr','/usr','--symlink','usr/lib64','/lib64','--symlink','usr/lib','/lib','--proc','/proc','--dev','/dev','--tmpfs','/tmp',
         '--ro-bind',str((a.images/(fs+'.img')).resolve()),'/image','--chdir','/',*tool]
    r=subprocess.run(cmd,capture_output=True,timeout=60)
    (a.images/(fs+'-check.log')).write_bytes(r.stdout+r.stderr)
    results.append({'filesystem':fs,'command':cmd,'returncode':r.returncode})
(a.images/'checks.json').write_text(json.dumps(results,indent=2)+'\n')
print(json.dumps(results,indent=2))
