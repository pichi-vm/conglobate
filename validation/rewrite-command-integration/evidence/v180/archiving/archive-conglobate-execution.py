from pathlib import Path
import shutil,json,hashlib,sys
p=Path('validation/rewrite-command-integration/evidence/v'+sys.argv[1])
for f in ['src/step.rs','src/step/execution.rs','src/step/execution/storage.rs','src/step/bootstrap.rs','src/step/finalize.rs']:
 t=p/'source'/f;t.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(f,t)
j=json.loads((p/'provenance.json').read_text())
for f in p.rglob('*'):
 if f.is_file() and f.name!='provenance.json':j['sha256'][str(f)]=hashlib.sha256(f.read_bytes()).hexdigest()
(p/'provenance.json').write_text(json.dumps(j,indent=2)+'\n')
for f,h in j['sha256'].items():assert hashlib.sha256(Path(f).read_bytes()).hexdigest()==h,f
print('execution evidence hashes verified')
