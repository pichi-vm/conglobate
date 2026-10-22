//! Restricted command writes on an owned, authenticated block-image fixture.
#![forbid(unsafe_code)]
use carapace::Assemble as _;
use conglobate::{mounts::Scratch, snapshot::Snapshot};
use std::{io::Write, error::Error, fs::{self, File}, os::unix::fs::chown, process::{Command, Stdio}};
fn main() -> Result<(), Box<dyn Error>> {
    if std::env::args().nth(1).as_deref() == Some("--export") { return export(); }
    let parent: [u8; 32] = hex::decode(fs::read_to_string("/base-root")?.trim())?
        .try_into().map_err(|_| "root length")?;
    let chain = carapace::Chain {
        root: parent.to_vec().into_boxed_slice(),
        scutes: vec![carapace::Scute { data: File::open("/dev/vda1")?, hash: File::open("/dev/vda2")? }],
    };
    let scratch = Scratch::new()?;
    let directory = scratch.path().join("layer");
    let snapshot = Snapshot::create(chain.assemble()?, parent, &directory)?;
    chown(snapshot.device_path(), Some(100000), Some(100000))?;
    let status = Command::new("/usr/bin/unshare")
        .env_clear().envs([("PATH", "/usr/bin:/usr/sbin"), ("HOME", "/"), ("LC_ALL", "C")])
        .args(["--user", "--mount", "--net", "--map-users", "0:100000:65536",
               "--map-groups", "0:100000:65536", "--setuid", "0", "--setgid", "0",
               "/bin/bash", "/chain-worker"])
        .arg(snapshot.device_path()).stdin(Stdio::null()).status()?;
    if !status.success() { return Err("restricted derivation failed".into()); }
    let layer = snapshot.finish()?;
    if layer.parent != parent || layer.root == parent { return Err("derived root identity".into()); }
    fs::create_dir("/work/export")?;
    fs::copy(layer.data_path(), "/work/export/data")?;
    fs::copy(directory.join("hash"), "/work/export/hash")?;
    fs::write("/work/export/root", hex::encode(layer.root))?;
    println!("DERIVED_BOOT_LAYER_SEALED");
    Ok(())
}

fn export() -> Result<(), Box<dyn Error>> {
    let mut entries = Vec::new();
    for name in ["data", "hash", "root"] {
        let path = std::path::PathBuf::from("/work/export").join(name);
        let (digest, size) = conglobate::scute::digest_file(&path)?;
        if size == 0 || size > 128 * 1024 * 1024 { return Err("export bound".into()); }
        entries.push((name, path, size, digest));
    }
    let header = entries.iter().map(|(name, _, size, digest)| {
        format!("{{\"name\":\"{name}\",\"bytes\":{size},\"sha256\":\"{}\"}}", &digest[7..])
    }).collect::<Vec<_>>().join(",");
    let port = std::env::args().nth(2).ok_or("export port required")?;
    let mut output = File::options().write(true).open(port)?;
    writeln!(output, "[{header}]")?;
    for (_, path, size, _) in entries {
        if std::io::copy(&mut File::open(path)?, &mut output)? != size { return Err("export changed".into()); }
    }
    output.flush()?;
    println!("DERIVED_BOOT_SERIAL_EXPORT_PASS");
    Ok(())
}
