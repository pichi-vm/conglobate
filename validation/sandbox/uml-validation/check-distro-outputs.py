#!/usr/bin/env python3
"""Independent read-only checks of completed owned distro root images."""
import argparse,hashlib,json,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--out',type=Path,required=True);p.add_argument('builds',type=Path,nargs='+');a=p.parse_args();out=a.out.resolve();out.mkdir(exist_ok=False);records=[]
for build in a.builds:
 r=json.loads((build/'result.json').read_text());assert r['pass'];disk=Path(r['output']);name=build.name
 def digest():
  with disk.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
 assert digest()==r['output_sha256'];entry={'build':str(build),'output':str(disk)};results={}
 for label,args in [('check',['e2fsck','-fn','/image']),('header',['dumpe2fs','-h','/image'])]:
  cmd=['bwrap','--unshare-all','--die-with-parent','--new-session','--cap-drop','ALL','--clearenv','--setenv','PATH','/usr/sbin:/usr/bin','--ro-bind','/usr','/usr','--symlink','usr/lib','/lib','--symlink','usr/lib64','/lib64','--proc','/proc','--dev','/dev','--tmpfs','/tmp','--ro-bind',str(disk),'/image','--chdir','/',*args]
  x=subprocess.run(cmd,capture_output=True,timeout=60);text=(x.stdout+x.stderr).decode(errors='replace');(out/(name+'-'+label+'.log')).write_text(text);results[label]={'code':x.returncode,'command':cmd}
  if label=='header':entry['state']=[l.split(':',1)[1].strip() for l in text.splitlines() if l.startswith('Filesystem state:')]
 entry['checks']=results;entry['unchanged']=digest()==r['output_sha256'];entry['pass']=all(x['code']==0 for x in results.values()) and entry['unchanged'] and entry['state']==['clean'];records.append(entry);print(json.dumps(entry),flush=True)
(out/'result.json').write_text(json.dumps({'cases':records,'pass':all(r['pass'] for r in records)},indent=2)+'\n')
