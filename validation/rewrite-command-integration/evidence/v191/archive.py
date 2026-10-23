from pathlib import Path
import shutil,json,hashlib,tarfile,subprocess
repo=Path('.');run=Path('/var/tmp/conglobate-command-integration-v191');here=Path('validation/rewrite-command-integration');dest=here/'evidence/v191';dest.mkdir(parents=True,exist_ok=True)
assert json.loads((run/'verdict.json').read_text())['passed']
assert b'COPY_MOUNTED_VOLUME_METADATA_PASS' in (run/'serial.txt').read_bytes()
for name in ['serial.txt','verdict.json','command.json','inputs.json','build.txt']:
    shutil.copyfile(run/name,dest/name)
for name in ['init','fixture.sh','workspace-context-commit']:
    shutil.copyfile(run/'root'/name,dest/(name+'.txt'))
subprocess.run(['git','-C',str(run/'root/workspace-context'),'bundle','create',str((dest/'context.bundle').resolve()),'--all'],check=True)
shutil.copytree(run/'root/cache-fixture',dest/'cache-fixture',dirs_exist_ok=True)
with tarfile.open(dest/'source.tar.gz','w:gz') as tar:
    for path in ['src','policy','tests','Cargo.toml','Cargo.lock','build.rs','DESIGN.md','SANDBOX-DESIGN.txt','ARCHITECTURE.md',str(here/'src'),str(here/'Cargo.toml'),str(here/'Cargo.lock'),str(here/'run.py'),str(here/'cache_fixture.py'),str(here/'pipeline_fixture.py')]:
        tar.add(path)
for p in Path('/var/tmp').glob('conglobate-m6-copy-writer-*.log'):shutil.copyfile(p,dest/p.name)
shutil.copyfile('/var/tmp/conglobate-v191-run.log',dest/'run.log');shutil.copyfile(__file__,dest/'archive.py')
(dest/'note.txt').write_text('v191 full normal disposable-VM suite plus mounted COPY check. Source archive includes the compile-fail documentation added after VM compilation; no production behavior changed. COPY assertion checks committed bytes, UID/GID/mode before final unmount/seal. No complete COPY recipe, bootstrap destination routing, or full Docker compatibility claim. Kernel/initramfs/helper remain external with hashes below.\n')
paths=[p for p in dest.rglob('*') if p.is_file() and p.name!='provenance.json']+[run/'vmlinuz',run/'initramfs.gz',run/'root/idmap-probe']
hashes={}
for p in paths:
    with p.open('rb') as f:hashes[str(p)]=hashlib.file_digest(f,'sha256').hexdigest()
(dest/'provenance.json').write_text(json.dumps({'base_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'sha256':hashes,'full_sandbox_qualified':False},indent=2)+'\n')
