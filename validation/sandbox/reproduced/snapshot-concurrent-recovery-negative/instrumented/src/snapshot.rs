// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Writable, disposable device-mapper snapshots with explicit finalization.

use crate::{
    Error,
    scute::{Scute, new_file, trim_cow},
};
use devmap_core::{BlockSize, Span};
use devmap_linux::{Control, DevId, Table, records::Records as _, targets::Target as _};
use devmap_snapshot::{Discard, Info, Parameters, Persistence, Persistent, Snapshot as Target};
use std::{
    fs::{File, OpenOptions},
    num::NonZeroU32,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);
// Carapace exposes a fixed 64-GiB logical origin. Sparse COW capacity allows
// every chunk to change, including exception metadata; tmpfs limits actual RAM.
const SECTORS: u64 = 64 * 1024 * 1024 * 1024 / 512;
const COW_CAPACITY: u64 = 65 * 1024 * 1024 * 1024;

#[derive(Debug)]
pub struct Snapshot {
    control: Control,
    id: DevId,
    device: Option<File>,
    cow_loop: Option<lodown::Device>,
    origin: Option<File>,
    cow: PathBuf,
    hash: PathBuf,
    parent: [u8; 32],
    deferred: bool,
}

impl Snapshot {
    pub fn create(origin: File, parent: [u8; 32], directory: &Path) -> Result<Self, Error> {
        std::fs::create_dir(directory)?;
        let cow = directory.join("data");
        let backing = new_file(&cow)?;
        backing.set_len(COW_CAPACITY)?;
        let attached = lodown::Control::open()?.attach(
            &backing,
            4096,
            lodown::Writable {
                autoclear: true,
                ..Default::default()
            },
        )?;
        probe_stop("attached");
        let target = Target::from(Parameters {
            origin: DevId::try_from(origin.metadata()?)?,
            cow: DevId::try_from(attached.as_ref().metadata()?)?,
            persistence: Persistence::Persistent(Persistent::Standard),
            chunk_size: BlockSize::from_bytes(NonZeroU32::new(4096).expect("nonzero constant"))
                .expect("valid constant"),
            discard: Discard::Disabled,
        });
        let control = Control::open()?;
        let publication_lock = File::open("/dev/mapper/control")?;
        publication_lock.lock()?;
        let lease = directory
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix("conglobate-"))
            .ok_or_else(|| {
                Error::Invalid("snapshot must belong to a conglobate scratch lease".into())
            })?;
        let name = format!(
            "conglobate-{}-{}-{}",
            std::process::id(),
            lease,
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        );
        let id = control.create(&name)?;
        probe_stop("created");
        let mut snapshot = Self {
            control,
            id,
            device: None,
            cow_loop: Some(attached),
            origin: Some(origin),
            cow,
            hash: directory.join("hash"),
            parent,
            deferred: false,
        };
        let table = Table::<4096>::default().add(
            Span::new(0, SECTORS),
            &target,
            snapshot.control.version(Target::NAME)?,
        )?;
        snapshot.control.load(id, table, false)?;
        probe_stop("loaded");
        snapshot.control.suspend(id, false)?;
        probe_stop("resumed");
        snapshot.device = Some(
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(snapshot.device_path())?,
        );
        probe_stop("opened");
        // Kernel holders retain this mapping. Process exit automatically removes
        // it after its mounts and descriptors close, including on cancellation.
        snapshot.control.remove(id, true)?;
        probe_stop("deferred");
        snapshot.deferred = true;
        Ok(snapshot)
    }

    pub fn device_path(&self) -> PathBuf {
        PathBuf::from(format!("/dev/dm-{}", self.id.minor()))
    }

    /// After the filesystem has been unmounted, flush and validate the COW,
    /// remove the mapping, detach the loop, then seal immutable bytes.
    pub fn finish(mut self) -> Result<Scute, Error> {
        if let Some(device) = &self.device {
            device.sync_all()?;
        }
        let mut rows = self.control.info::<4096>(self.id)?;
        let row = rows
            .next()
            .ok_or_else(|| Error::Invalid("snapshot has no status".into()))??;
        let info = row.parse::<Target>(self.control.version(Target::NAME)?)?;
        if !matches!(info, Info::Usage { .. }) {
            return Err(Error::Invalid(format!("snapshot failed: {info:?}")));
        }
        drop(rows);
        self.control.suspend(self.id, true)?;
        self.device.take();
        self.cow_loop.take();
        self.origin.take();
        let mut file = OpenOptions::new().read(true).write(true).open(&self.cow)?;
        trim_cow(&mut file)?;
        drop(file);
        Scute::seal(self.cow.clone(), self.hash.clone(), self.parent)
    }
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        self.device.take();
        // Best-effort rollback is also required during partially failed setup.
        // If still held by the kernel, deferred removal owns final teardown.
        if !self.deferred {
            let _ = self.control.remove(self.id, true);
        }
        self.cow_loop.take();
        self.origin.take();
    }
}

/// Recover an activation interrupted before deferred removal was installed.
/// The caller holds the dead scratch directory's exclusive lifetime lock.
pub(crate) fn recover(lease: &str) -> Result<(), Error> {
    let lock = File::open("/dev/mapper/control")?;
    lock.lock()?;
    let control = Control::open()?;
    let mut devices = control.devices::<65_536>()?;
    while let Some(device) = devices.next() {
        let (name, id) = device?;
        let fields: Vec<_> = name.split('-').collect();
        if fields.len() != 4
            || fields[0] != "conglobate"
            || fields[2] != lease
            || fields[1].parse::<u32>().is_err()
            || fields[3].parse::<u64>().is_err()
        {
            continue;
        }
        if let Ok((current, status)) = control.by_name(name) {
            // Linux dm-ioctl.h: status exposes DM_DEFERRED_REMOVE (bit 17).
            // Already-deferred mappings own their teardown; do not race their
            // automatic deletion and minor reuse. An interrupted activation
            // without this flag cannot disappear while cooperating creators
            // and recoverers hold the publication lock. It may have no /dev
            // node yet, so recovery must not depend on opening that node.
            const DEFERRED_REMOVE: u32 = 1 << 17;
            if current == id && status.flags & DEFERRED_REMOVE == 0 {
                control.remove(id, true)?;
            }
        }
    }
    Ok(())
}

// Test-only instrumentation in an isolated copied crate.
fn probe_stop(stage: &str) {
    if std::env::var("SNAPSHOT_STOP").as_deref() == Ok(stage) {
        std::fs::write("/work/reached", stage).unwrap();
        nix::sys::signal::raise(nix::sys::signal::Signal::SIGSTOP).unwrap();
        panic!("stopped fixture unexpectedly resumed");
    }
}
