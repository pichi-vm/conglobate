from pathlib import Path
import shutil,json,hashlib,sys
p=Path('validation/rewrite-command-integration/evidence/v'+sys.argv[1])
for f in ['src/isolation/receiver.rs','src/storage/volume.rs','src/storage/bootstrap.rs','src/storage.rs','src/step/bootstrap.rs','src/step/finalize.rs','src/step.rs','src/filesystem.rs','src/filesystem/attachment.rs','src/process/supervision/registration.rs','src/process/supervision/volume.rs','validation/rewrite-command-integration/src/bootstrap.rs']:
 t=p/'source'/f;t.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(f,t)
j=json.loads((p/'provenance.json').read_text())
for f in p.rglob('*'):
 if f.is_file() and f.name!='provenance.json':j['sha256'][str(f)]=hashlib.sha256(f.read_bytes()).hexdigest()
(p/'provenance.json').write_text(json.dumps(j,indent=2)+'\n')
for f,h in j['sha256'].items():assert hashlib.sha256(Path(f).read_bytes()).hexdigest()==h,f
print('bootstrap evidence hashes verified')
