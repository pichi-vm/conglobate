from pathlib import Path
import shutil,subprocess,hashlib,json,sys
n=sys.argv[1]; out=Path('/var/tmp/conglobate-command-integration-v'+n); dst=Path('validation/rewrite-command-integration/evidence/v'+n); dst.mkdir()
for name in ('serial.txt','verdict.json','command.json','inputs.json','build.txt'):
 if (out/name).exists(): shutil.copy2(out/name,dst/name)
for src in ['src/storage/verity.rs','src/storage.rs','src/cache/chain/prepared.rs','src/chain.rs','validation/rewrite-command-integration/src/main.rs','validation/rewrite-command-integration/src/readiness.rs','validation/rewrite-command-integration/run.py','validation/rewrite-command-integration/cache_fixture.py','Cargo.toml','Cargo.lock','validation/rewrite-command-integration/Cargo.toml','validation/rewrite-command-integration/Cargo.lock']:
 target=dst/'source'/src; target.parent.mkdir(parents=True,exist_ok=True); shutil.copy2(src,target)
for name,src in [('fixture.sh.txt','fixture.sh'),('init.txt','init'),('workspace-context-commit.txt','workspace-context-commit')]: shutil.copy2(out/'root'/src,dst/name)
shutil.copytree(out/'root/cache-fixture',dst/'cache-fixture')
subprocess.run(['git','-C',str(out/'root/workspace-context'),'bundle','create',str((dst/'context.bundle').resolve()),'--all'],check=True)
(dst/'review-note.txt').write_text(sys.argv[2]+'\n')
paths=[p for p in dst.rglob('*') if p.is_file()]+[out/'vmlinuz',out/'initramfs.gz',out/'root/idmap-probe']
(dst/'provenance.json').write_text(json.dumps({'base_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'full_artifacts':str(out),'sha256':{str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths},'full_sandbox_qualified':False},indent=2)+'\n')
