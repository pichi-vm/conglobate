// SPDX-License-Identifier: Apache-2.0
use conglobate::{
    filesystem::MountPoint,
    isolation::{CommandDevices, PrivateMountNamespace},
    process::Supervision,
    step::SetupCutoff,
};
use std::{fs, io::Write, os::unix::net::UnixDatagram, path::Path, time::Duration};

#[derive(Debug)]
pub(super) struct BlockedIo;
impl BlockedIo {
    pub(super) fn helper(cutoff: &std::ffi::OsStr) -> Result<(), Box<dyn std::error::Error>> {
        let cutoff = SetupCutoff::from_argument(cutoff.to_str().ok_or("non-UTF8 cutoff")?)?;
        let deadline = cutoff.deadline()?;
        let channel = Supervision::from_stdin()?
            .exchange_until(deadline)?
            .join_domain_until(deadline)?;
        let input = channel.receive_volume_until(deadline)?;
        let scope = PrivateMountNamespace::enter()?;
        let (view, _setup, resolver) =
            input.prepare(&scope, CommandDevices::create(100000, 100000)?)?;
        assert!(resolver.is_none());
        let _view = view
            .attach(MountPoint::open(Path::new(
                "/work/runtime-parent/receiver-view",
            ))?)
            .map_err(|e| e.to_string())?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .open("/work/runtime-parent/receiver-view/blocked-writer")?;
        let release = UnixDatagram::bind("/work/runtime-parent/blocked-release.sock")?;
        release.set_read_timeout(Some(Duration::from_secs(10)))?;
        channel.report_prepared_until(deadline)?;
        let _channel = channel.await_admission_until(deadline)?;
        release.send_to(
            &std::process::id().to_ne_bytes(),
            "/work/runtime-parent/blocked-ready.sock",
        )?;
        let mut packet = [0; 2];
        assert_eq!(release.recv(&mut packet)?, 2);
        assert_eq!(&packet, b"go");
        // Only the fixture-owned snapshot in this disposable VM is suspended.
        file.write_all(&[0x5a; 4096])?;
        file.sync_all()?;
        Err("fixture write unexpectedly completed while device suspended".into())
    }
}
