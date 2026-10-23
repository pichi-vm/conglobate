from pathlib import Path
import json,hashlib,shutil,subprocess
here=Path('validation/rewrite-command-integration')
for n in [187,188]:
    run=Path(f'/var/tmp/conglobate-command-integration-v{n}')
    dest=here/'evidence'/f'v{n}'
    dest.mkdir(parents=True,exist_ok=True)
    for name in ['serial.txt','verdict.json','command.json','inputs.json','build.txt','pipeline-build.txt','pipeline-index.json','pipeline-manifest.json','pipeline-inventory.cbor']:
        if (run/name).exists(): shutil.copyfile(run/name,dest/name)
    for name in ['fixture.sh','pipeline-commit','pipeline-recipe-committed']:
        shutil.copyfile(run/'root'/name,dest/name)
    for name in ['cache-fixture','pipeline-context']:
        shutil.copytree(run/'root'/name,dest/name,dirs_exist_ok=True)
    paths=list(Path('src').rglob('*.rs'))+[Path('src/formatter/ext4.conf'),Path('Cargo.toml'),Path('Cargo.lock'),Path('build.rs'),Path('DESIGN.md'),Path('SANDBOX-DESIGN.txt'),Path('ARCHITECTURE.md')]
    paths+=list((here/'src').rglob('*.rs'))+[here/name for name in ['Cargo.toml','Cargo.lock','run.py','cache_fixture.py','pipeline_fixture.py']]
    for source in paths:
        target=dest/'source'/source; target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source,target)
    if n==187:
        source=dest/'source'/here/'src/pipeline.rs';text=source.read_text()
        start=text.index('        println!("PIPELINE_INDEX_HEX')
        end=text.index('        assert!(',start)
        source.write_text(text[:start]+text[end:])
        source=dest/'source'/here/'run.py';text=source.read_text()
        start=text.index("    for name in ['INDEX', 'INVENTORY', 'MANIFEST']:")
        end=text.index('    passed = all(marker in serial',start)
        source.write_text(text[:start]+text[end:])
    note='Production sources unchanged between v187 and v188. v187 fixture source reconstructed by reversing only the documented artifact-export insertion; v188 source captured directly. DESIGN.md includes the transform-identity clarification made during v187. Neither run is full command/distro/hardware qualification.\n'
    (dest/'review-note.txt').write_text(note)
    for log in ['check','clippy','lib','doc','fixture-clippy']:
        shutil.copyfile(f'/var/tmp/conglobate-m6-pipeline-{log}.log',dest/f'{log}.log')
    shutil.copyfile(f'/var/tmp/conglobate-v{n}-run.log',dest/'run.log')
    shutil.copyfile('/var/tmp/archive-conglobate-pipeline.py',dest/'archive.py')
    paths=sorted(p for p in dest.rglob('*') if p.is_file() and p.name!='provenance.json')
    paths += [run/'vmlinuz',run/'initramfs.gz',run/'root/idmap-probe',run/'root/usr/bin/conglobate']
    hashes={}
    for p in paths:
        with p.open('rb') as f: hashes[str(p)]=hashlib.file_digest(f,'sha256').hexdigest()
    (dest/'provenance.json').write_text(json.dumps({'base_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'sha256':hashes,'full_sandbox_qualified':False},indent=2)+'\n')
    print(n,len(hashes),'files/artifacts preserved')
manifest=Path('/var/tmp/conglobate-command-integration-v188/pipeline-manifest.json').read_bytes()
index=json.loads(Path('/var/tmp/conglobate-command-integration-v188/pipeline-index.json').read_text())
assert index['manifests'][0]['digest']=='sha256:'+hashlib.sha256(manifest).hexdigest()
assert index['manifests'][0]['size']==len(manifest)
print('Exported index/manifest identity verified')
