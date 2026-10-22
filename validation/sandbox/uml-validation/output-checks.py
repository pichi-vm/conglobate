#!/usr/bin/env python3
"""Bounded ordinary missing/truncated/unclean output fixtures; never host-mounted."""
import argparse,hashlib,json,shutil,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--out',type=Path,required=True);a=p.parse_args();out=a.out.resolve();out.mkdir(exist_ok=False)
base=out/'clean.img'
with base.open('xb') as f:f.truncate(32*1024*1024)
subprocess.run(['mkfs.ext4','-F','-q',str(base)],check=True)
shutil.copyfile(base,out/'truncated.img')
with (out/'truncated.img').open('r+b') as f:f.truncate(2048)
shutil.copyfile(base,out/'unclean.img')
subprocess.run(['debugfs','-w','-R','set_super_value state 0',str(out/'unclean.img')],check=True,capture_output=True)
results=[]
for name in ['clean','missing','truncated','unclean']:
 image=out/(name+'.img');expected=name=='clean';record={'case':name,'expected_accept':expected}
 if not image.exists():record.update(accepted=False,reason='missing')
 else:
  before=hashlib.sha256(image.read_bytes()).hexdigest();records={}
  for label,args in [('check',['e2fsck','-fn','/image']),('header',['dumpe2fs','-h','/image'])]:
   cmd=['bwrap','--unshare-all','--die-with-parent','--new-session','--cap-drop','ALL','--clearenv','--setenv','PATH','/usr/sbin:/usr/bin','--ro-bind','/usr','/usr','--symlink','usr/lib','/lib','--symlink','usr/lib64','/lib64','--proc','/proc','--dev','/dev','--tmpfs','/tmp','--ro-bind',str(image),'/image','--chdir','/',*args]
   r=subprocess.run(cmd,capture_output=True,timeout=15);log=r.stdout+r.stderr;(out/(name+'-'+label+'.log')).write_bytes(log);records[label]={'returncode':r.returncode,'command':cmd,'text':log.decode(errors='replace')}
  state=[l.split(':',1)[1].strip() for l in records['header']['text'].splitlines() if l.startswith('Filesystem state:')]
  record.update(accepted=all(r['returncode']==0 for r in records.values()) and state==['clean'],state=state,unchanged=hashlib.sha256(image.read_bytes()).hexdigest()==before,codes={k:v['returncode'] for k,v in records.items()})
 record['pass']=record['accepted']==expected and record.get('unchanged',True);results.append(record)
(out/'result.json').write_text(json.dumps({'cases':results,'pass':all(r['pass'] for r in results)},indent=2)+'\n');print(json.dumps(results))
