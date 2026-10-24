// SPDX-License-Identifier: Apache-2.0
use conglobate::{
    filesystem::MountPoint,
    isolation::{
        CommandDevices, CommandReceiver, PrivateMountNamespace, receiver::ReceiverFailure,
    },
    process::Supervision,
    step::SetupCutoff,
};
use std::{fs, io, os::unix::net::UnixDatagram, path::Path, process::Command, time::Duration};

#[derive(Debug)]
pub(super) struct GateFailure;
impl GateFailure {
    pub(super) fn receiver(cutoff: &std::ffi::OsStr) -> Result<(), Box<dyn std::error::Error>> {
        let cutoff = SetupCutoff::from_argument(cutoff.to_str().ok_or("non-UTF8 cutoff")?)?;
        let deadline = cutoff.deadline()?;
        let channel = Supervision::from_stdin()?
            .exchange_until(deadline)?
            .join_domain_until(deadline)?;
        let input = channel.receive_volume_until(deadline)?;
        let scope = PrivateMountNamespace::enter()?;
        let (view, setup, resolver) =
            input.prepare(&scope, CommandDevices::create(100000, 100000)?)?;
        assert!(resolver.is_none());
        let view = view
            .attach(MountPoint::open(Path::new(
                "/work/runtime-parent/receiver-view",
            ))?)
            .map_err(|e| e.to_string())?;
        let mut child = Command::new(std::env::current_exe()?);
        child.arg("sandbox-gate-child");
        let failure = CommandReceiver::new(channel, view, setup)
            .execute(child, cutoff)
            .unwrap_err();
        match *failure {
            ReceiverFailure::Cleaned(error) => assert_eq!(error.kind(), io::ErrorKind::BrokenPipe),
            other => return Err(format!("expected clean start-gate failure: {other}").into()),
        }
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        assert_eq!(
            fs::read_dir("/work/runtime-parent/receiver-view")?.count(),
            0
        );
        scope.restore().map_err(|e| e.to_string())?;
        fs::write("/work/receiver-gate-cleaned", "cleaned")?;
        Ok(())
    }

    pub(super) fn child() -> Result<(), Box<dyn std::error::Error>> {
        use rustix::net::{
            self, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendFlags,
        };
        use std::{io::IoSliceMut, mem::MaybeUninit};
        // Fixture-only receiver for our own trusted CommandReceiver offer.
        // No descriptor API is added to production and no image code executes.
        let mut bytes = [0; 4];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let received = net::recvmsg(
            io::stdin(),
            &mut [IoSliceMut::new(&mut bytes)],
            &mut ancillary,
            RecvFlags::CMSG_CLOEXEC | RecvFlags::DONTWAIT,
        )?;
        let mut descriptors = Vec::new();
        for message in ancillary.drain() {
            match message {
                RecvAncillaryMessage::ScmRights(rights) => descriptors.extend(rights),
                _ => return Err("unexpected fixture ancillary message".into()),
            }
        }
        assert_eq!(received.bytes, 3);
        assert_eq!(&bytes[..3], &[0x82, 1, 2]);
        assert!(
            !received
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
        );
        assert_eq!(descriptors.len(), 3);
        let gate = descriptors.pop().unwrap();
        drop(descriptors);
        let release = UnixDatagram::bind("/work/runtime-parent/gate-release.sock")?;
        release.set_read_timeout(Some(Duration::from_secs(10)))?;
        // Shut down reception before readiness. The receiver can read Ready,
        // but its later Start send must fail, irrespective of scheduling.
        net::shutdown(&gate, net::Shutdown::Read)?;
        assert_eq!(net::send(&gate, &[0x82, 1, 1], SendFlags::NOSIGNAL)?, 3);
        let mut command = [0; 4];
        assert_eq!(release.recv(&mut command)?, 4);
        assert_eq!(&command, b"exit");
        fs::remove_file("/work/runtime-parent/gate-release.sock")?;
        eprintln!("GATE_FIXTURE_CHILD_RELEASED");
        Ok(())
    }
}
