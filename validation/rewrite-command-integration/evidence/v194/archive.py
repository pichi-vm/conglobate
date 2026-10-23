from pathlib import Path
import shutil,json,hashlib,tarfile,subprocess,sys
n=int(sys.argv[1])
repo=Path('.');run=Path(f'/var/tmp/conglobate-command-integration-v{n}');here=Path('validation/rewrite-command-integration');dest=here/f'evidence/v{n}';dest.mkdir(parents=True,exist_ok=True)
for name in ['serial.txt','verdict.json','command.json','inputs.json','build.txt']:
    shutil.copyfile(run/name,dest/name)
for name in ['init','fixture.sh','workspace-context-commit']:
    shutil.copyfile(run/'root'/name,dest/(name+'.txt'))
subprocess.run(['git','-C',str(run/'root/workspace-context'),'bundle','create',str((dest/'context.bundle').resolve()),'--all'],check=True)
shutil.copytree(run/'root/cache-fixture',dest/'cache-fixture',dirs_exist_ok=True)
with tarfile.open(dest/'source.tar.gz','w:gz') as tar:
    for path in ['src','policy','tests','Cargo.toml','Cargo.lock','build.rs','DESIGN.md','SANDBOX-DESIGN.txt','ARCHITECTURE.md',str(here/'src'),str(here/'Cargo.toml'),str(here/'Cargo.lock'),str(here/'run.py'),str(here/'cache_fixture.py'),str(here/'pipeline_fixture.py')]:
        tar.add(path)
for p in Path('/var/tmp').glob('conglobate-m6-directives-*.log'):shutil.copyfile(p,dest/p.name)
shutil.copyfile(f'/var/tmp/conglobate-v{n}-run.log',dest/'run.log');shutil.copyfile(__file__,dest/'archive.py')
(dest/'note.txt').write_text('v194 full normal disposable-VM suite executes parsed RUN/COPY directives through production CommandSandbox::directives, including CARAPACE. Source snapshot is the exact run fixture before removal of an unnecessary explicit drop(non-Drop) that fixture Clippy rejected. No production behavior changes in that lint correction. CLI stage coordinator and full platform qualification remain open.\n')
paths=[p for p in dest.rglob('*') if p.is_file() and p.name!='provenance.json']+[run/'vmlinuz',run/'initramfs.gz',run/'root/idmap-probe']
hashes={}
for p in paths:
    with p.open('rb') as f:hashes[str(p)]=hashlib.file_digest(f,'sha256').hexdigest()
(dest/'provenance.json').write_text(json.dumps({'base_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'sha256':hashes,'full_sandbox_qualified':False},indent=2)+'\n')
