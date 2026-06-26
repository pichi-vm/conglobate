// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! The in-guest build (BUILD.md §5), driven by conglobate after init.
//!
//!   * 8a — establish the two virtio-fs channels (`context` read-only,
//!     `output` writable) and write the output manifest (`build.yaml`).
//!   * 8b — attach the source carapace (the recipe `from:`) as a composed
//!     read-only origin and flatten it into the base scute (cow + verity).
//!   * 8c — per `carapace.yaml` directive: a writable dm-snapshot over the
//!     composed previous layer (COW store = a loop device over a /tmp tmpfs
//!     file), mount + apply the directive, then re-emit the changes as a COW
//!     (`write_delta`) and a dm-verity tree salt-chained onto the prior root.
//!
//! No `unsafe`: every syscall is a rustix safe wrapper, carapace assembly /
//! scute emission come from the workspace's no-unsafe crates, and the
//! chrooted `run:` directive runs in a re-exec of conglobate itself (so the
//! `chroot(2)` is in a throwaway child — see [`exec_in_root`]).

use std::fs;
use std::io::Read as _;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use carapace_dm::{
    DmCreateMode, DmDevice, DmTable, LoopDevice, TableLine, TargetSpec, open_dm_control,
};
use carapace_import::verity;
use conglobate::{BuildOutput, CopyDirective, Directive, FromSpec, ScuteOut};
use rustix::mount::{MountFlags, UnmountFlags, mount, unmount};
use sha2::{Digest as _, Sha256};

/// virtio-fs mount tags the host and conglobate agree on. Mirror of the
/// constants in `pichi/src/cmd/build.rs` — a host↔guest contract.
const CONTEXT_TAG: &str = "context";
const OUTPUT_TAG: &str = "output";

/// Where the read-only build context and writable output sink are mounted.
const CONTEXT_DIR: &str = "/context";
const OUTPUT_DIR: &str = "/output";
/// Where each directive's writable snapshot is mounted for modification.
const WORK_DIR: &str = "/work";

/// COW chunk size for emitted scutes + the dm-snapshots conglobate builds:
/// the carapace-mandated value (the carapace read side rejects anything else).
const SCUTE_CHUNK_SIZE_SECTORS: u32 = carapace_import::SCUTE_CHUNK_SIZE_SECTORS;

/// dm-verity block sizes for emitted scutes (RDP-locked at 4096).
const VERITY_BLOCK_SIZE: u32 = 4096;

/// Sparse size of each per-snapshot COW backing file on the `/tmp` tmpfs.
/// Generous: only the dm-snapshot chunks a directive actually writes are
/// committed to RAM (the file is sparse, the read back is bounded by the
/// snapshot's allocated extent — see [`emit_delta_scute`]). Replaces the
/// former brd ramdisk, whose fixed size capped a layer's change set; no
/// real-world kernel ships both erofs and brd, so the COW store is now a
/// loop device over this file.
const COW_MAX_BYTES: u64 = 1 << 30;

/// Filesystem type conglobate mounts a carapace's working snapshot as. MVP:
/// carapace content is a bare filesystem (matching the carapace read tests),
/// and ext4 is built into the build kernel.
const ROOTFS_TYPE: &str = "ext4";

/// Re-exec arg: conglobate spawned as `conglobate __exec-in-root <root> <cmd>`
/// chroots into `<root>` and runs `<cmd>` via `sh -c`. Lets the `chroot(2)`
/// happen in a throwaway child instead of mutating PID 1's root.
pub(crate) const EXEC_IN_ROOT_ARG: &str = "__exec-in-root";

/// Drive the build. Returns `Err` on a genuine build failure (conglobate
/// powers off regardless; the host detects the failure by the absent or
/// empty `output/build.yaml`).
///
/// When no `context` share is attached (e.g. the module-only boot tests),
/// this is a no-op success — there is nothing to build.
pub(crate) fn run() -> Result<(), String> {
    let Some(context) = mount_context() else {
        eprintln!("conglobate: no `{CONTEXT_TAG}` share attached; nothing to build");
        return Ok(());
    };
    let build_dir = context.join("pichi.build");

    let recipe = conglobate::CarapaceRecipe::parse(&read(&build_dir.join("carapace.yaml"))?)
        .map_err(|e| format!("parsing carapace.yaml: {e}"))?;
    let refs = conglobate::RefsLock::parse(&read(&build_dir.join("refs.lock"))?)
        .map_err(|e| format!("parsing refs.lock (run `pichi update`): {e}"))?;
    let source_root = refs
        .get(&recipe.from)
        .and_then(|e| e.carapace.strip_prefix("sha256:"))
        .ok_or_else(|| format!("refs.lock has no carapace root for `{}`", recipe.from))?
        .to_string();
    eprintln!(
        "conglobate: building from {} (root {source_root})",
        recipe.from
    );

    let output = mount_output()?;

    // Attach the source carapace as the composed read-only origin. Per the
    // carapace spec this device's apparent size is huge (ZERO_COUNT); we
    // never read it whole — each directive's delta comes from its own
    // bounded dm-snapshot COW store.
    let origin = carapace::attach("source", &source_root)
        .map_err(|e| format!("attaching source carapace (root {source_root}): {e}"))?;

    // The source carapace's scutes pass through unchanged (the host prepends
    // them when packaging); conglobate emits only the per-directive delta
    // scutes, salt-chained onto the source's top root.
    let mut parent_root = decode_root(&source_root)?;
    let mut scutes: Vec<ScuteOut> = Vec::new();

    // Per directive (BUILD.md §5): a writable dm-snapshot over the composed
    // previous layer (COW store = a loop device over a /tmp tmpfs file), mount
    // + apply, then the live COW *is* the layer's change set — emit it as the
    // cow and compute the salt-chained verity tree.
    let mut control = open_dm_control().map_err(|e| format!("opening /dev/mapper/control: {e}"))?;
    let mut origin_path = origin.clone();
    for (i, directive) in recipe.derive.iter().enumerate() {
        let cow = make_cow(i)?;
        let cow_dev = cow.path().to_path_buf();
        let snap = create_snapshot(&mut control, &format!("build{i}"), &origin_path, &cow_dev)?;
        let snap_path = snap.dev_node();

        mount_rootfs(&snap_path)?;
        let res = apply_directive(directive, &context, Path::new(WORK_DIR), None);
        rustix::fs::sync();
        unmount(WORK_DIR, UnmountFlags::empty()).map_err(|e| format!("unmount {WORK_DIR}: {e}"))?;
        res?;
        rustix::fs::sync();

        let salt = parent_root.to_vec();
        let (scute, root) = emit_delta_scute(&mut control, &snap, &cow_dev, &salt, &output, i)?;
        scutes.push(scute);
        parent_root = root;

        // The composed snapshot view becomes the next directive's origin.
        // Leak the dm device *and* its loop-backed COW so they persist past
        // this scope and back the next layer (PID 1 powers off, so there is
        // nothing to clean up).
        origin_path = snap_path;
        snap.forget();
        cow.forget();
    }

    // PMI build (BUILD.md §2.2): if pmi.yaml is present, run its directives in
    // a chroot of the PMI source and retain the single file it writes
    // (`into:`). The output carapace's top root is exported so the author's
    // `arma build --cmdline "… root.carapace=$PICHI_CARAPACE_ROOT"` binds it.
    let carapace_root = hex::encode(parent_root);
    let pmi = build_pmi(
        &build_dir,
        &context,
        &refs,
        &BuiltCarapace {
            origin: &origin_path,
            root: &carapace_root,
        },
        &mut control,
        &output,
        recipe.derive.len(),
    )?;

    let manifest = BuildOutput { scutes, pmi };
    let yaml = manifest
        .to_yaml()
        .map_err(|e| format!("serializing build.yaml: {e}"))?;
    fs::write(output.join("build.yaml"), yaml).map_err(|e| format!("writing build.yaml: {e}"))?;
    Ok(())
}

/// chroot into `root` and run `cmd` via `sh -c`. Invoked in the re-exec child
/// (`conglobate __exec-in-root <root> <cmd>`) so the `chroot(2)` never touches
/// PID 1. Never returns on success (it `exec`s the shell); returns `Err` only
/// if the chroot or spawn fails.
pub(crate) fn exec_in_root(root: &str, cmd: &str) -> Result<(), String> {
    std::env::set_current_dir(root).map_err(|e| format!("chdir {root}: {e}"))?;
    rustix::process::chroot(root).map_err(|e| format!("chroot {root}: {e}"))?;
    std::env::set_current_dir("/").map_err(|e| format!("chdir / after chroot: {e}"))?;
    let status = Command::new("/bin/sh")
        .arg("-c")
        .arg(cmd)
        .status()
        .map_err(|e| format!("spawn /bin/sh -c: {e}"))?;
    std::process::exit(status.code().unwrap_or(1));
}

/// Emit one directive's delta scute. After the directive ran against the
/// mounted snapshot, the snapshot's COW store (`cow_dev`, a loop device over a
/// sparse `/tmp` file) holds exactly the chunks the directive changed, already
/// in dm-snapshot persistent format — that *is* the delta scute's cow. Read
/// back only the snapshot's *allocated* extent (queried from dm status), not
/// the full sparse device, then compute the dm-verity tree salted with the
/// parent root. Returns the [`ScuteOut`] and this scute's root.
///
/// MVP note: copies the live COW verbatim rather than the canonical write-once
/// re-emit of BUILD.md §5.2 (determinism + minimal size). The bytes are a
/// valid scute cow either way; the re-emit is a later refinement.
fn emit_delta_scute(
    control: &mut std::fs::File,
    snap: &DmDevice,
    cow_dev: &Path,
    salt: &[u8],
    output: &Path,
    index: usize,
) -> Result<(ScuteOut, [u8; 32]), String> {
    let (allocated_sectors, _total) = snap
        .snapshot_allocated(control)
        .map_err(|e| format!("snapshot {index} status: {e}"))?;
    let cow_bytes = read_cow_prefix(cow_dev, allocated_sectors)?;
    write_scute_blobs(&cow_bytes, salt, output, index)
}

/// Compute the dm-verity tree over `cow_bytes` (salt = chain prefix), write
/// both blobs (`<index>.cow`, `<index>.verity`) to the output sink, and
/// return the [`ScuteOut`] + verity root.
fn write_scute_blobs(
    cow_bytes: &[u8],
    salt: &[u8],
    output: &Path,
    index: usize,
) -> Result<(ScuteOut, [u8; 32]), String> {
    let cow_digest: [u8; 32] = Sha256::digest(cow_bytes).into();
    let params = verity::VerityParams {
        data_block_size: VERITY_BLOCK_SIZE,
        hash_block_size: VERITY_BLOCK_SIZE,
        salt: salt.to_vec(),
        uuid: verity::derive_uuid(salt, &cow_digest),
    };
    let vout = verity::compute(cow_bytes, &params).map_err(|e| format!("verity::compute: {e}"))?;

    let cow_name = format!("{index:04}.cow");
    let verity_name = format!("{index:04}.verity");
    fs::write(output.join(&cow_name), cow_bytes).map_err(|e| format!("writing {cow_name}: {e}"))?;
    fs::write(output.join(&verity_name), &vout.blob)
        .map_err(|e| format!("writing {verity_name}: {e}"))?;

    Ok((
        ScuteOut {
            cow: cow_name,
            verity: verity_name,
            salt: hex::encode(salt),
        },
        vout.root_hash,
    ))
}

/// The carapace conglobate just built: its composed read-only origin device
/// and its top dm-verity root (hex). The fallback PMI base when `pmi.yaml`
/// has no `from:`, and the value exported as `PICHI_CARAPACE_ROOT`.
struct BuiltCarapace<'a> {
    origin: &'a Path,
    root: &'a str,
}

/// Run `pmi.yaml` (if present) to seal the application PMI (BUILD.md §2.2).
///
/// The PMI build runs in a chroot of the PMI source — `pmi.yaml`'s `from:` (a
/// kernel-builder carapace) or, if omitted, the carapace just built
/// (`carapace_origin`). Its `derive:` directives run with the build context
/// bound at `/context` and `PICHI_CARAPACE_ROOT` exported; the single file at
/// `into:` is copied to the output sink as `boot.pmi`. Nothing else is
/// retained. Returns `Some("boot.pmi")` when a PMI was built, else `None`.
fn build_pmi(
    build_dir: &Path,
    context: &Path,
    refs: &conglobate::RefsLock,
    carapace: &BuiltCarapace<'_>,
    control: &mut std::fs::File,
    output: &Path,
    cow_index: usize,
) -> Result<Option<String>, String> {
    let carapace_origin = carapace.origin;
    let carapace_root = carapace.root;
    let pmi_yaml = build_dir.join("pmi.yaml");
    if !pmi_yaml.is_file() {
        return Ok(None);
    }
    let recipe = conglobate::PmiRecipe::parse(&read(&pmi_yaml)?)
        .map_err(|e| format!("parsing pmi.yaml: {e}"))?;

    // PMI build base: pmi.yaml `from:` (attach it) or the just-built carapace.
    let pmi_origin = match &recipe.from {
        Some(reference) => {
            let root = refs
                .get(reference)
                .and_then(|e| e.carapace.strip_prefix("sha256:"))
                .ok_or_else(|| format!("refs.lock has no root for pmi.yaml from `{reference}`"))?;
            carapace::attach("pmisrc", root)
                .map_err(|e| format!("attaching pmi source `{reference}` ({root}): {e}"))?
        }
        None => carapace_origin.to_path_buf(),
    };

    let cow = make_cow(cow_index)?;
    let cow_dev = cow.path().to_path_buf();
    let snap = create_snapshot(control, "pmibuild", &pmi_origin, &cow_dev)?;
    let snap_path = snap.dev_node();
    let work = Path::new(WORK_DIR);

    mount_rootfs(&snap_path)?;
    // Binds persist across all PMI directives + the copy-out (unlike the
    // per-directive carapace path) so `into:` under a bound /tmp survives.
    let result = (|| -> Result<(), String> {
        mount_chroot_binds(work, context)?;
        let run = (|| -> Result<(), String> {
            for directive in &recipe.derive {
                match directive {
                    Directive::Run(cmd) => exec_directive_in_root(work, cmd, Some(carapace_root))?,
                    Directive::Copy(c) => apply_copy(c, context, work)?,
                }
            }
            // Retain only the file at `into:`.
            let into_rel = recipe.into.trim_start_matches('/');
            let pmi_bytes = fs::read(work.join(into_rel))
                .map_err(|e| format!("reading pmi `into:` {}: {e}", recipe.into))?;
            fs::write(output.join("boot.pmi"), &pmi_bytes)
                .map_err(|e| format!("writing boot.pmi: {e}"))?;
            Ok(())
        })();
        umount_chroot_binds(work);
        run
    })();
    rustix::fs::sync();
    let _ = unmount(WORK_DIR, UnmountFlags::empty());
    snap.forget();
    cow.forget();
    result?;
    Ok(Some("boot.pmi".to_string()))
}

/// Build a writable dm-snapshot named `name` over `origin_path`, with its COW
/// exception store on `cow_path` (a loop device over a `/tmp` tmpfs file).
/// Returns the resumed device.
fn create_snapshot(
    control: &mut std::fs::File,
    name: &str,
    origin_path: &Path,
    cow_path: &Path,
) -> Result<DmDevice, String> {
    let origin = dev_of(origin_path)?;
    let cow = dev_of(cow_path)?;
    let sectors = block_sectors(origin_path)?;
    let dev = DmDevice::create(control, name, DmCreateMode::ReadWrite)
        .map_err(|e| format!("dm create {name}: {e}"))?;
    let table = DmTable {
        lines: vec![TableLine {
            start: 0,
            length: sectors,
            target: TargetSpec::Snapshot {
                origin,
                cow,
                chunk_size_sectors: u64::from(SCUTE_CHUNK_SIZE_SECTORS),
            },
        }],
    };
    dev.load_table(control, &table)
        .map_err(|e| format!("dm load_table {name}: {e}"))?;
    dev.resume(control)
        .map_err(|e| format!("dm resume {name}: {e}"))?;
    Ok(dev)
}

/// Apply one directive to the mounted working filesystem at `root`.
/// `carapace_root` (the output carapace's top root) is exported as
/// `PICHI_CARAPACE_ROOT` for `run:` directives (PMI builds reference it on the
/// kernel cmdline — BUILD.md §11, env not magic injection).
fn apply_directive(
    directive: &Directive,
    context: &Path,
    root: &Path,
    carapace_root: Option<&str>,
) -> Result<(), String> {
    match directive {
        Directive::Run(cmd) => {
            mount_chroot_binds(root, context)?;
            let res = exec_directive_in_root(root, cmd, carapace_root);
            umount_chroot_binds(root);
            res
        }
        Directive::Copy(c) => apply_copy(c, context, root),
    }
}

/// The pseudo-filesystems + build context bound into a chroot for `run:`.
const CHROOT_BINDS: [&str; 4] = ["proc", "sys", "dev", "tmp"];

/// Bind `/proc`,`/sys`,`/dev`,`/tmp` and the read-only build context (at
/// `/context`) into the working root, so a chrooted command sees them. The
/// context bind lets PMI builds reference build inputs (kernel, initrd, tools)
/// without copying them into the rootfs (which would bloat the snapshot COW).
fn mount_chroot_binds(root: &Path, context: &Path) -> Result<(), String> {
    for b in CHROOT_BINDS {
        let target = root.join(b);
        let _ = fs::create_dir_all(&target);
        mount(
            format!("/{b}"),
            &target,
            "",
            MountFlags::BIND | MountFlags::REC,
            None,
        )
        .map_err(|e| format!("bind /{b} -> {}: {e}", target.display()))?;
    }
    let ctx_target = root.join("context");
    let _ = fs::create_dir_all(&ctx_target);
    mount(
        context,
        &ctx_target,
        "",
        MountFlags::BIND | MountFlags::REC,
        None,
    )
    .map_err(|e| {
        format!(
            "bind {} -> {}: {e}",
            context.display(),
            ctx_target.display()
        )
    })?;
    Ok(())
}

/// Tear down the binds from [`mount_chroot_binds`] (best-effort, lazy).
fn umount_chroot_binds(root: &Path) {
    let _ = unmount(
        root.join("context").to_str().unwrap_or("/"),
        UnmountFlags::DETACH,
    );
    for b in CHROOT_BINDS.iter().rev() {
        let _ = unmount(root.join(b).to_str().unwrap_or("/"), UnmountFlags::DETACH);
    }
}

/// chroot+exec `cmd` in a re-exec child (so PID 1 keeps its root). Binds must
/// already be mounted. `carapace_root` is exported as `PICHI_CARAPACE_ROOT`.
fn exec_directive_in_root(
    root: &Path,
    cmd: &str,
    carapace_root: Option<&str>,
) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let mut command = Command::new(exe);
    command.arg(EXEC_IN_ROOT_ARG).arg(root).arg(cmd);
    if let Some(root_hex) = carapace_root {
        command.env("PICHI_CARAPACE_ROOT", root_hex);
    }
    let status = command
        .status()
        .map_err(|e| format!("spawn chroot child: {e}"))?;
    if !status.success() {
        return Err(format!("run `{cmd}` exited with {status}"));
    }
    Ok(())
}

/// Apply a `copy:` directive: install the source path(s) from the build
/// context into the working root at `into`. MVP scope: file copies; the
/// owner/group/mode metadata is applied when present and numeric.
fn apply_copy(c: &CopyDirective, context: &Path, root: &Path) -> Result<(), String> {
    let into_rel = c.into.trim_start_matches('/');
    match &c.from {
        FromSpec::One(src) => {
            let dest = root.join(into_rel);
            copy_one(&context.join(src), &dest)?;
            apply_metadata(c, &dest)?;
        }
        FromSpec::Many(srcs) => {
            // `into` is a directory; install each source under it by basename.
            let dest_dir = root.join(into_rel);
            fs::create_dir_all(&dest_dir)
                .map_err(|e| format!("mkdir {}: {e}", dest_dir.display()))?;
            for src in srcs {
                let name = Path::new(src)
                    .file_name()
                    .ok_or_else(|| format!("copy source has no file name: {src}"))?;
                let dest = dest_dir.join(name);
                copy_one(&context.join(src), &dest)?;
                apply_metadata(c, &dest)?;
            }
        }
    }
    Ok(())
}

/// Copy one file, creating parent directories.
fn copy_one(src: &Path, dest: &Path) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    fs::copy(src, dest)
        .map(|_| ())
        .map_err(|e| format!("copy {} -> {}: {e}", src.display(), dest.display()))
}

/// Apply `mode` (and numeric `owner`/`group`) from a copy directive. Name
/// resolution against the parent scute's `/etc/passwd` is deferred (MVP);
/// non-numeric owner/group are skipped with a warning.
fn apply_metadata(c: &CopyDirective, dest: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt as _;
    if let Some(mode) = &c.mode {
        let bits = u32::from_str_radix(mode.trim_start_matches("0o"), 8)
            .map_err(|e| format!("invalid mode {mode:?}: {e}"))?;
        fs::set_permissions(dest, fs::Permissions::from_mode(bits))
            .map_err(|e| format!("chmod {}: {e}", dest.display()))?;
    }
    let numeric = |v: &Option<String>| v.as_ref().and_then(|s| s.parse::<u32>().ok());
    if let (Some(uid), gid) = (numeric(&c.owner), numeric(&c.group)) {
        rustix::fs::chown(
            dest,
            Some(rustix::fs::Uid::from_raw(uid)),
            gid.map(rustix::fs::Gid::from_raw),
        )
        .map_err(|e| format!("chown {}: {e}", dest.display()))?;
    } else if c
        .owner
        .as_deref()
        .is_some_and(|s| s.parse::<u32>().is_err())
    {
        eprintln!(
            "conglobate: skipping non-numeric owner {:?} (name resolution deferred)",
            c.owner
        );
    }
    Ok(())
}

/// Mount the working snapshot device at [`WORK_DIR`].
fn mount_rootfs(dev: &Path) -> Result<(), String> {
    let _ = fs::create_dir(WORK_DIR);
    mount(
        dev.to_str().unwrap_or_default(),
        WORK_DIR,
        ROOTFS_TYPE,
        MountFlags::empty(),
        None,
    )
    .map_err(|e| format!("mount {} -> {WORK_DIR} ({ROOTFS_TYPE}): {e}", dev.display()))
}

/// `(major, minor)` of a device node. Decodes dev_t per `<linux/kdev_t.h>`
/// (same split as `carapace_dm`'s internal helper).
fn dev_of(path: &Path) -> Result<(u32, u32), String> {
    let rdev = fs::metadata(path)
        .map_err(|e| format!("stat {}: {e}", path.display()))?
        .rdev();
    let major = ((rdev >> 8) & 0xfff) as u32;
    let minor = ((rdev & 0xff) | ((rdev >> 12) & 0xfff00)) as u32;
    Ok((major, minor))
}

/// Block-device size in 512-byte sectors, from
/// `/sys/class/block/<name>/size`.
fn block_sectors(path: &Path) -> Result<u64, String> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("no device name in {}", path.display()))?;
    let sysfs = format!("/sys/class/block/{name}/size");
    read(Path::new(&sysfs))?
        .trim()
        .parse::<u64>()
        .map_err(|e| format!("parsing {sysfs}: {e}"))
}

/// Acquire a fresh RAM-backed block device for one snapshot's COW exception
/// store: a sparse [`COW_MAX_BYTES`] file on the `/tmp` tmpfs attached to a
/// loop device (via `carapace_dm`). tmpfs commits only the chunks the
/// dm-snapshot actually writes, so the generous cap costs nothing until used.
/// Replaces the former brd ramdisk; the caller leaks the returned device so it
/// outlives the build (PID 1 powers off — nothing to clean up).
fn make_cow(index: usize) -> Result<LoopDevice, String> {
    let backing = PathBuf::from(format!("/tmp/cow{index}.img"));
    let file = fs::File::create(&backing)
        .map_err(|e| format!("creating COW backing {}: {e}", backing.display()))?;
    file.set_len(COW_MAX_BYTES)
        .map_err(|e| format!("sizing COW backing {}: {e}", backing.display()))?;
    drop(file);
    LoopDevice::attach(&backing).map_err(|e| format!("loop-attaching {}: {e}", backing.display()))
}

/// Read the in-use prefix of a snapshot COW device: `allocated_sectors`
/// (from dm status) rounded up to a whole chunk, plus one chunk of slack, so
/// the emitted scute cow holds the full exception store without copying the
/// sparse tail of the loop device.
fn read_cow_prefix(path: &Path, allocated_sectors: u64) -> Result<Vec<u8>, String> {
    let chunk = u64::from(SCUTE_CHUNK_SIZE_SECTORS);
    let sectors = allocated_sectors
        .div_ceil(chunk)
        .saturating_add(1)
        .saturating_mul(chunk);
    let len = sectors.saturating_mul(512).min(COW_MAX_BYTES);
    let len = usize::try_from(len).map_err(|_| format!("COW prefix {len} too large"))?;
    let mut f = fs::File::open(path).map_err(|e| format!("opening {}: {e}", path.display()))?;
    let mut buf = vec![0u8; len];
    f.read_exact(&mut buf)
        .map_err(|e| format!("reading {} ({len} bytes): {e}", path.display()))?;
    Ok(buf)
}

/// Decode a 32-byte verity root from its lowercase-hex form (the carapace
/// salt-chain anchor for the first delta scute).
fn decode_root(hex_root: &str) -> Result<[u8; 32], String> {
    let bytes = hex::decode(hex_root).map_err(|e| format!("source root not hex: {e}"))?;
    bytes
        .try_into()
        .map_err(|v: Vec<u8>| format!("source root is {} bytes, expected 32", v.len()))
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))
}

/// Mount the read-only `context` share. `None` if no such device is attached
/// (mount fails) — distinguishes "no build requested" from a real error in a
/// way the module-only boot tests rely on.
fn mount_context() -> Option<PathBuf> {
    let dir = Path::new(CONTEXT_DIR);
    let _ = fs::create_dir(dir);
    match mount(CONTEXT_TAG, dir, "virtiofs", MountFlags::RDONLY, None) {
        Ok(()) => Some(dir.to_path_buf()),
        Err(e) => {
            eprintln!("conglobate: mount {CONTEXT_TAG} -> {CONTEXT_DIR}: {e}");
            None
        }
    }
}

/// Mount the writable `output` sink. A build with a context but no output
/// sink is a host wiring error — fail.
fn mount_output() -> Result<PathBuf, String> {
    let dir = Path::new(OUTPUT_DIR);
    let _ = fs::create_dir(dir);
    mount(OUTPUT_TAG, dir, "virtiofs", MountFlags::empty(), None)
        .map_err(|e| format!("mount {OUTPUT_TAG} -> {OUTPUT_DIR}: {e}"))?;
    Ok(dir.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scute writer must produce an attachable COW + verity pair and a
    /// deterministic root (recomputing verity over the written cow matches).
    #[test]
    fn write_scute_blobs_is_deterministic_and_on_disk() {
        let tmp = tempfile::TempDir::new().unwrap();
        let out = tmp.path();
        let cow_bytes = carapace_import::cow::write(
            &{
                let mut v = vec![0u8; 4096 * 3];
                v[5000] = 0x42;
                v
            },
            SCUTE_CHUNK_SIZE_SECTORS,
        )
        .unwrap();
        let salt = vec![0u8; 32];

        let (scute, root) = write_scute_blobs(&cow_bytes, &salt, out, 0).unwrap();
        assert_eq!(scute.cow, "0000.cow");
        assert_eq!(scute.salt, "00".repeat(32));

        let on_disk = std::fs::read(out.join("0000.cow")).unwrap();
        let cow_digest: [u8; 32] = Sha256::digest(&on_disk).into();
        let params = verity::VerityParams {
            data_block_size: VERITY_BLOCK_SIZE,
            hash_block_size: VERITY_BLOCK_SIZE,
            salt: salt.clone(),
            uuid: verity::derive_uuid(&salt, &cow_digest),
        };
        assert_eq!(verity::compute(&on_disk, &params).unwrap().root_hash, root);
    }

    /// A chained scute salts with the parent root, so its verity root differs
    /// from the same content salted as a base — the salt-chain binding.
    #[test]
    fn chained_salt_changes_the_root() {
        let tmp = tempfile::TempDir::new().unwrap();
        let out = tmp.path();
        let cow_bytes =
            carapace_import::cow::write(&vec![1u8; 4096 * 2], SCUTE_CHUNK_SIZE_SECTORS).unwrap();

        let (_, base_root) = write_scute_blobs(&cow_bytes, &[0u8; 32], out, 0).unwrap();
        let (_, child_root) = write_scute_blobs(&cow_bytes, &base_root, out, 1).unwrap();
        assert_ne!(
            base_root, child_root,
            "salt-chain must bind the parent root"
        );
    }
}
