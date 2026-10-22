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
    path = project / 'src/artifact.rs'
    source = path.read_text()
    before = hashlib.sha256(path.read_bytes()).hexdigest()
    anchors = {
        'before-check': ('', '        copy_tree(&self.directory, staging.path())?;'),
        'copy': ('            output.sync_all()?;', '                output.write_all(&buffer[..count])?;'),
        'before-rename': ('', '        rustix::fs::renameat_with('),
        'after-rename': ('', '        .map_err(std::io::Error::from)?;'),
    }
    for stage, (_, anchor) in anchors.items():
        assert source.count(anchor) == 1, stage
        hook = f'        probe_stop("{stage}");'
        if stage == 'before-rename':
            source = source.replace(anchor, hook + '\n' + anchor)
        else:
            source = source.replace(anchor, anchor + '\n' + hook)
    source += '\n// Test-only gates in an isolated source copy.\nfn probe_stop(stage: &str) {\n    if std::env::var("ARTIFACT_STOP").as_deref() == Ok(stage) {\n        std::fs::write("/work/reached", stage).unwrap();\n        nix::sys::signal::raise(nix::sys::signal::Signal::SIGSTOP).unwrap();\n        if std::env::var_os("ARTIFACT_CANCEL_RESUME").is_some() {\n            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);\n            while crate::cancel::check().is_ok() {\n                assert!(std::time::Instant::now() < deadline, "cancellation signal not handled");\n                std::thread::sleep(std::time::Duration::from_millis(1));\n            }\n            println!("ARTIFACT_GATE_CANCEL_FLAG_OBSERVED {stage}");\n        } else {\n            panic!("stopped fixture unexpectedly resumed");\n        }\n    }\n}\n'
    path.write_text(source)
    (project / 'src/bin').mkdir(exist_ok=True)
    shutil.copy2(fixture / 'helper.rs', project / 'src/bin/artifact-death.rs')
    command = ['cargo', 'build', '--locked', '--manifest-path', str(project / 'Cargo.toml'),
               '--target-dir', str(target), '--bin', 'artifact-death']
    with (out / 'artifact-build.log').open('wb') as log:
        subprocess.run(command, check=True, stdout=log, stderr=subprocess.STDOUT)
    binary = target / 'debug/artifact-death'
    catalog = {'command': command, 'original_artifact_sha256': before,
               'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
               'sources': {str(p.relative_to(project)): hashlib.sha256(p.read_bytes()).hexdigest()
                           for p in sorted(project.rglob('*')) if p.is_file()}}
    (out / 'artifact-build-inputs.json').write_text(json.dumps(catalog, indent=2)+'\n')
    return binary
