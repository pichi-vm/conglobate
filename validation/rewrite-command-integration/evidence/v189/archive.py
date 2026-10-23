from pathlib import Path
import json,hashlib,shutil,subprocess,sys
n=int(sys.argv[1]); here=Path('validation/rewrite-command-integration'); run=Path(f'/var/tmp/conglobate-command-integration-v{n}'); dest=here/'evidence'/f'v{n}';dest.mkdir(parents=True,exist_ok=True)
for name in ['serial.txt','verdict.json','command.json','inputs.json','build.txt','pipeline-build.txt','pipeline-index.json','pipeline-manifest.json','pipeline-inventory.cbor']:
    if (run/name).exists():shutil.copyfile(run/name,dest/name)
for name in ['fixture.sh','pipeline-commit','pipeline-recipe-committed']:
    shutil.copyfile(run/'root'/name,dest/name)
for name in ['cache-fixture','pipeline-context']:
    shutil.copytree(run/'root'/name,dest/name,dirs_exist_ok=True)
(dest/'pipeline-context/.git').rename(dest/'pipeline-context/git-metadata')
paths=list(Path('src').rglob('*.rs'))+[Path(p) for p in ['Cargo.toml','Cargo.lock','build.rs','DESIGN.md','SANDBOX-DESIGN.txt']]
paths+=list((here/'src').rglob('*.rs'))+[here/p for p in ['Cargo.toml','Cargo.lock','run.py','cache_fixture.py','pipeline_fixture.py']]
for p in paths:
    target=dest/'source'/p;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,target)
for p in Path('/var/tmp').glob('conglobate-m6-transform-bind-*.log'):shutil.copyfile(p,dest/p.name)
shutil.copyfile(f'/var/tmp/conglobate-v{n}-run.log',dest/'run.log');shutil.copyfile(__file__,dest/'archive.py')
(dest/'note.txt').write_text('Git metadata stored as git-metadata to avoid nested repository staging; restore as .git. Source snapshot includes test-only assembly fixture additions after v189 compilation; production code and integration fixture correspond to run. No distro/boot/platform qualification.\n')
paths=[p for p in dest.rglob('*') if p.is_file() and p.name!='provenance.json']+[run/'vmlinuz',run/'initramfs.gz',run/'root/idmap-probe',run/'root/usr/bin/conglobate']
hashes={}
for p in paths:
    with p.open('rb') as f:hashes[str(p)]=hashlib.file_digest(f,'sha256').hexdigest()
(dest/'provenance.json').write_text(json.dumps({'base_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'sha256':hashes},indent=2)+'\n')
