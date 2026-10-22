"""Build an isolated source copy with stop gates; never patch production files."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess


def build(repository, fixture, out, target):
    project = out / 'instrumented'
    project.mkdir()
    for name in ('Cargo.toml', 'Cargo.lock'):
        shutil.copy2(repository / name, project / name)
    shutil.copytree(repository / 'src', project / 'src')
    path = project / 'src/snapshot.rs'
    source = path.read_text()
    before = hashlib.sha256(path.read_bytes()).hexdigest()
    gates = {
        'attached': '        let target = Target::from(Parameters {',
        'created': '        let mut snapshot = Self {',
        'loaded': '        snapshot.control.suspend(id, false)?;',
        'resumed': '        snapshot.device = Some(',
        'opened': '        // Kernel holders retain this mapping.',
        'deferred': '        snapshot.deferred = true;',
    }
    for stage, anchor in gates.items():
        assert source.count(anchor) == 1, (stage, anchor)
        source = source.replace(anchor, f'        probe_stop("{stage}");\n' + anchor)
    source += '''
// Test-only instrumentation in an isolated copied crate.
fn probe_stop(stage: &str) {
    if std::env::var("SNAPSHOT_STOP").as_deref() == Ok(stage) {
        std::fs::write("/work/reached", stage).unwrap();
        nix::sys::signal::raise(nix::sys::signal::Signal::SIGSTOP).unwrap();
        panic!("stopped fixture unexpectedly resumed");
    }
}
'''
    path.write_text(source)
    mounts = project / 'src/mounts.rs'
    text = mounts.read_text()
    anchor = '        // A live Scratch may remove its directory before releasing its'
    assert text.count(anchor) == 1
    hook = '        // Test-only deterministic lease-disappearance gate.\n        if std::env::var_os("RECOVERY_READ_GATE").is_some() {\n            std::fs::write("/work/read-gate", entry.path().as_os_str().as_encoded_bytes())?;\n            nix::sys::signal::raise(nix::sys::signal::Signal::SIGSTOP)?;\n            if std::env::var_os("RECOVERY_OLD_READ").is_some() {\n                // Exact former lookup expression, retained as negative control.\n                if std::fs::read_dir(entry.path())?.next().is_some() {\n                    continue;\n                }\n            }\n        }\n'
    mounts.write_text(text.replace(anchor, hook + anchor))
    (project / 'src/bin').mkdir(exist_ok=True)
    shutil.copy2(fixture / 'helper.rs', project / 'src/bin/snapshot-death.rs')
    command = ['cargo', 'build', '--locked', '--manifest-path', str(project / 'Cargo.toml'),
               '--target-dir', str(target), '--bin', 'snapshot-death']
    with (out / 'snapshot-build.log').open('wb') as log:
        subprocess.run(command, check=True, stdout=log, stderr=subprocess.STDOUT)
    binary = target / 'debug/snapshot-death'
    catalog = {'command': command, 'original_snapshot_sha256': before,
               'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
               'sources': {str(p.relative_to(project)): hashlib.sha256(p.read_bytes()).hexdigest()
                           for p in sorted(project.rglob('*')) if p.is_file()}}
    (out / 'snapshot-build-inputs.json').write_text(json.dumps(catalog, indent=2)+'\n')
    return binary
