//! Committed input and actual inventory/publication integration for chain.rs.
use conglobate::{
    artifact::Artifact,
    cache::{Cache, ResolvedImage},
    input::Inputs,
    inventory::{Digest, Inventory, Record},
    recipe::Directive,
    scute::{Chain, digest_file},
};
use std::{error::Error, fs, path::Path, process::Command};

const REPOSITORY: &str = "/work/repository";
const RECIPE: &str = "from: base\nderive:\n  - run: /command\n";

fn git(args: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = Command::new("git")
        .args(["-C", REPOSITORY])
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", "/work")
        .env("GIT_AUTHOR_NAME", "Validation")
        .env("GIT_AUTHOR_EMAIL", "validation@example.invalid")
        .env("GIT_COMMITTER_NAME", "Validation")
        .env("GIT_COMMITTER_EMAIL", "validation@example.invalid")
        .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z")
        .output()?;
    assert!(output.status.success(), "git: {:?}", output);
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

pub fn prepare(cache_path: &Path, base: &ResolvedImage) -> Result<Inputs, Box<dyn Error>> {
    let manifest = &base.manifest_bytes;
    let manifest_digest = Digest::sha256(manifest).oci();
    fs::write(
        cache_path.join("blobs/sha256").join(&manifest_digest[7..]),
        manifest,
    )?;
    let index = format!(
        r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.index.v1+json","manifests":[{{"mediaType":"application/vnd.oci.image.manifest.v1+json","artifactType":"application/vnd.pichi.artifact.v1+json","digest":"{manifest_digest}","size":{},"platform":{{"os":"pichi","architecture":"amd64"}}}}]}}"#,
        manifest.len()
    );
    let index_digest = Digest::sha256(index.as_bytes()).oci();
    fs::write(
        cache_path.join("blobs/sha256").join(&index_digest[7..]),
        index,
    )?;
    fs::create_dir_all(Path::new(REPOSITORY).join("pichi.build"))?;
    fs::write(
        Path::new(REPOSITORY).join("pichi.build/carapace.yaml"),
        RECIPE,
    )?;
    fs::write(
        Path::new(REPOSITORY).join("pichi.build/refs.lock"),
        format!("version: 1\nimages:\n  base:\n    index: {index_digest}\n"),
    )?;
    git(&["init", "-q", "--template=", "--initial-branch=validation"])?;
    git(&["add", "."])?;
    git(&["-c", "commit.gpgsign=false", "commit", "-qm", "fixture"])?;
    let commit = git(&["rev-parse", "HEAD"])?;
    let inputs = Inputs::resolve(
        Path::new(REPOSITORY),
        &commit,
        Path::new("."),
        &Cache::new(cache_path),
    )?;
    assert_eq!(inputs.images.len(), 1);
    assert_eq!(inputs.images.values().next().unwrap().index, index_digest);
    let steps = &inputs
        .pipeline
        .carapace
        .as_ref()
        .ok_or("no carapace")?
        .derive;
    assert!(
        matches!(steps.as_slice(), [Directive::Run { command, network: false }] if command == "/command")
    );
    // The committed objects are authoritative. This invalid working-tree edit
    // must not change the already captured execution recipe or its inventory.
    fs::write(
        Path::new(REPOSITORY).join("pichi.build/carapace.yaml"),
        "from: absent\n",
    )?;
    println!("COMMITTED_LOCKED_CHAIN_INPUTS_PASS {commit} {index_digest}");
    Ok(inputs)
}

pub fn publish(
    inputs: &Inputs,
    chain: &Chain,
    image: &ResolvedImage,
    scratch: &Path,
) -> Result<(), Box<dyn Error>> {
    let order: Vec<_> = chain
        .scutes
        .iter()
        .map(|s| s.descriptor.digest.clone())
        .collect();
    let inventory = Record {
        inputs,
        output: &image.manifest,
        repository: Path::new(REPOSITORY),
        subdirectory: Path::new("."),
        before_transform: None,
        scute_order: &order,
    }
    .inventory()?;
    assert_eq!(
        inventory.execution.recipes["carapace.yaml"],
        Digest::sha256(RECIPE.as_bytes())
    );
    assert_eq!(inventory.inputs[0].roles, ["carapace-base"]);
    assert_eq!(inventory.outputs.scutes[0].origin, "inherited");
    assert!(
        inventory.outputs.scutes[1..]
            .iter()
            .all(|s| s.origin == "produced")
    );
    assert_eq!(
        inventory.inputs[0]
            .carapace_root
            .as_deref()
            .map(Vec::as_slice),
        Some(chain.scutes[0].root.as_slice())
    );
    assert_eq!(
        inventory
            .outputs
            .carapace_root
            .as_deref()
            .map(Vec::as_slice),
        Some(chain.root()?.as_slice())
    );
    assert_eq!(
        inventory.builder.executable.oci(),
        digest_file(Path::new("/proc/self/exe"))?.0
    );
    let artifact = Artifact::new(&scratch.join("artifact"))?;
    for scute in &chain.scutes {
        let (digest, size) = artifact.put(fs::File::open(scute.data_path())?)?;
        assert_eq!(
            (digest.oci(), size),
            (scute.descriptor.digest.clone(), scute.descriptor.size)
        );
    }
    artifact.finalize(&image.manifest, &inventory)?;
    if std::env::args().any(|arg| arg == "--cancel-publication") {
        conglobate::cancel::install()?;
        let private_inventory = fs::read(scratch.join("artifact/inventory.cbor"))?;
        assert_eq!(
            Inventory::decode(&private_inventory)?.encode()?,
            inventory.encode()?
        );
        println!("CHAIN_PRIVATE_INVENTORY_FINALIZED_BEFORE_CANCEL");
        // Trusted diagnostic child signals only its parent (this probe).
        assert!(
            Command::new("/bin/bash")
                .args(["-c", "kill -TERM \"$PPID\""])
                .status()?
                .success()
        );
        assert!(conglobate::cancel::check().is_err());
        let error = artifact
            .publish(Path::new("/work/published"))
            .expect_err("cancelled build published");
        assert!(error.to_string().contains("build cancelled"), "{error}");
        assert!(!Path::new("/work/published").exists());
        assert!(!fs::read_dir("/work")?.any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".conglobate-output-")
        }));
        assert_eq!(
            fs::read(scratch.join("artifact/inventory.cbor"))?,
            private_inventory
        );
        println!("CHAIN_CANCELLED_PUBLICATION_NO_OUTPUT_OR_TEMP_PASS");
        return Ok(());
    }
    artifact.publish(Path::new("/work/published"))?;
    let bytes = fs::read("/work/published/inventory.cbor")?;
    assert_eq!(Inventory::decode(&bytes)?.encode()?, inventory.encode()?);
    let published_cache = Cache::new("/work/published/image.oci");
    published_cache.verify_blobs(&image.manifest)?;
    let rebuilt = Chain::stage(&published_cache, image, &scratch.join("published-chain"))?;
    assert_eq!(rebuilt.root()?, chain.root()?);
    assert_eq!(rebuilt.scutes.len(), chain.scutes.len());
    assert_eq!(
        hex::encode(&inventory.context.commit),
        inputs.context.commit
    );
    assert_eq!(hex::encode(&inventory.context.tree), inputs.context.tree);
    println!("CHAIN_COMMITTED_INVENTORY_PUBLICATION_PASS");
    Ok(())
}
