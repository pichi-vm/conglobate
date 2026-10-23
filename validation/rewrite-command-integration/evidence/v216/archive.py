from pathlib import Path
import shutil,json,hashlib,tarfile,subprocess,sys
n=int(sys.argv[1])
repo=Path('.');run=Path(f'/var/tmp/conglobate-command-integration-v{n}');here=Path('validation/rewrite-command-integration');dest=here/f'evidence/v{n}';dest.mkdir(parents=True,exist_ok=True)
for name in ['serial.txt','verdict.json','command.json','inputs.json','build.txt','pipeline-build.txt']:
    shutil.copyfile(run/name,dest/name)
for name in ['init','fixture.sh','workspace-context-commit']:
    shutil.copyfile(run/'root'/name,dest/(name+'.txt'))
subprocess.run(['git','-C',str(run/'root/workspace-context'),'bundle','create',str((dest/'context.bundle').resolve()),'--all'],check=True)
shutil.copytree(run/'root/cache-fixture',dest/'cache-fixture',dirs_exist_ok=True)
with tarfile.open(dest/'source.tar.gz','w:gz') as tar:
    for path in ['src','policy','tests','Cargo.toml','Cargo.lock','build.rs','DESIGN.md','SANDBOX-DESIGN.txt','ARCHITECTURE.md',str(here/'src'),str(here/'Cargo.toml'),str(here/'Cargo.lock'),str(here/'run.py'),str(here/'cache_fixture.py'),str(here/'pipeline_fixture.py')]:
        tar.add(path)
for p in Path('/var/tmp').glob('conglobate-m6-veth-*.log'):shutil.copyfile(p,dest/p.name)
shutil.copyfile(f'/var/tmp/conglobate-v{n}-run.log',dest/'run.log');shutil.copyfile(__file__,dest/'archive.py')
(dest/'note.txt').write_text('v216 PASS: focused disposable VM production route-netlink/veth resource test. Kernel accepts explicit locally administered MAC identifiers during exclusive pair creation; both interfaces start DOWN. Duplicate creation cleanup preserves the original owner. Explicit close removes both links and marker. Expired setup cleans without creating links. v212-v215 preserve failed IfAlias assumptions (v215 repeated v214 after an edit script failed). No NAT, resolver mount, command networking or full sandbox claim.\n')
paths=[p for p in dest.rglob('*') if p.is_file() and p.name!='provenance.json']+[run/'vmlinuz',run/'initramfs.gz',run/'root/idmap-probe',run/'root/usr/bin/conglobate']
hashes={}
for p in paths:
    with p.open('rb') as f:hashes[str(p)]=hashlib.file_digest(f,'sha256').hexdigest()
(dest/'provenance.json').write_text(json.dumps({'base_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'sha256':hashes,'full_sandbox_qualified':False},indent=2)+'\n')
