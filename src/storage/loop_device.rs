// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    marker::PhantomData,
    time::{Duration, Instant},
};

mod sealed {
    pub(crate) trait Sealed {}
}
pub(super) trait Access: sealed::Sealed {
    const READ_ONLY: bool;
    fn flush(file: &File) -> io::Result<()>;
}
#[derive(Debug)]
pub(super) struct ReadOnly;
#[derive(Debug)]
pub(super) struct Writable;
impl sealed::Sealed for ReadOnly {}
impl sealed::Sealed for Writable {}
impl Access for ReadOnly {
    const READ_ONLY: bool = true;
    fn flush(_: &File) -> io::Result<()> {
        Ok(())
    }
}
impl Access for Writable {
    const READ_ONLY: bool = false;
    fn flush(file: &File) -> io::Result<()> {
        file.sync_all()
    }
}

/// Storage-private loop authority. Failed/deferred close returns the same owner;
/// autoclear on descriptor drop is not a successful-detachment receipt.
#[derive(Debug)]
pub(super) struct LoopDevice<A: Access> {
    device: lodown::Device,
    _access: PhantomData<A>,
}
impl<A: Access> LoopDevice<A> {
    pub(super) fn attach(file: &File) -> io::Result<Self> {
        let length = file.metadata()?.len();
        if length == 0 || !length.is_multiple_of(4096) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "loop backing must contain a positive number of 4096-byte blocks",
            ));
        }
        let device = lodown::Device::open(lodown::Control::open()?.get_free()?)?;
        device.configure(
            file,
            4096,
            lodown::Configurable {
                read_only: A::READ_ONLY,
                writable: lodown::Writable {
                    autoclear: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        )?;
        Ok(Self {
            device,
            _access: PhantomData,
        })
    }
    pub(super) fn id(&self) -> io::Result<devmap_linux::DevId> {
        self.device.as_ref().metadata()?.try_into()
    }
    #[cfg(feature = "test-fixtures")]
    pub(super) fn number(&self) -> io::Result<u32> {
        Ok(self.device.status()?.number)
    }
    pub(super) fn close(self) -> Result<(), (io::Error, Self)> {
        let result = (|| -> io::Result<()> {
            // systemd-udevd holds LOCK_SH for a whole blkid probe and rearms
            // that probe on IN_CLOSE_WRITE. Device-mapper's last close of this
            // loop is one of those writes, so a probe is usually in progress
            // here and is a real extra opener. Non-blocking LOCK_EX, retried
            // until PROBE_LOCK_DEADLINE, waits that probe out and stops a new
            // one from opening the node. On deadline the error returns below
            // with this owner intact: nothing is cleared or dropped. An opener
            // that never takes the shared lock is unaffected. LOOP_CLR_FD
            // enters rundown only when this fd is the sole opener, and success
            // still requires ENXIO.
            acquire_exclusive_lock(self.device.as_ref())?;
            A::flush(self.device.as_ref())?;
            lodown::Device::from(self.device.as_ref().try_clone()?).clear()?;
            match self.device.status() {
                Err(error) if error.raw_os_error() == Some(6) => Ok(()), // ENXIO: unbound
                Err(error) => Err(error),
                Ok(_) => Err(io::Error::new(io::ErrorKind::ResourceBusy, LIVE_HOLDER)),
            }
        })();
        result.map_err(|source| (source, self))
    }
}

// Upper bound for draining a udev LOCK_SH probe before detach fails closed.
// The smoke log's probe window was ~25ms (unmount at 3.560s, live-holder at
// 3.585s). One second covers that blkid pass on a busy single-CPU guest.
// Linux flock has no writer preference, so a blocking LOCK_EX can wait
// forever while probes re-arm; this deadline fails closed instead.
const PROBE_LOCK_DEADLINE: Duration = Duration::from_secs(1);
const PROBE_LOCK_POLL: Duration = Duration::from_millis(20);
const LIVE_HOLDER: &str = "loop detach remains deferred by a live holder";

fn acquire_exclusive_lock(file: &File) -> io::Result<()> {
    let deadline = Instant::now() + PROBE_LOCK_DEADLINE;
    loop {
        // Before every attempt, including the retry after EINTR. A probe that
        // drops after this instant must not be acquired: sleeping out a
        // WouldBlock, or spinning on EINTR, both come back here first.
        if Instant::now() >= deadline {
            return Err(io::Error::new(io::ErrorKind::ResourceBusy, LIVE_HOLDER));
        }
        match rustix::fs::flock(file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(PROBE_LOCK_POLL),
            ),
            Err(rustix::io::Errno::INTR) => {}
            Err(error) => return Err(error.into()),
        }
    }
}
impl Read for LoopDevice<Writable> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.device.read(bytes)
    }
}
impl Write for LoopDevice<Writable> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.device.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        Writable::flush(self.device.as_ref())
    }
}
impl Seek for LoopDevice<Writable> {
    fn seek(&mut self, offset: SeekFrom) -> io::Result<u64> {
        self.device.seek(offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::OpenOptions,
        os::fd::AsRawFd,
        path::PathBuf,
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    };

    fn attach_writable() -> Option<LoopDevice<Writable>> {
        let backing = tempfile::tempfile().unwrap();
        backing.set_len(4096).unwrap();
        // EPERM/EACCES: rootless CI has no loop-control authority. The kernel
        // retains the backing reference taken by LOOP_CONFIGURE.
        match LoopDevice::<Writable>::attach(&backing) {
            Ok(device) => Some(device),
            Err(error) if matches!(error.raw_os_error(), Some(1) | Some(13)) => None,
            Err(error) => panic!("attach loop: {error}"),
        }
    }

    fn node_path(device: &LoopDevice<Writable>) -> PathBuf {
        std::fs::read_link(format!(
            "/proc/self/fd/{}",
            device.device.as_ref().as_raw_fd()
        ))
        .unwrap()
    }

    fn assert_unbound(path: &std::path::Path) {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .unwrap();
        let error = lodown::Device::from(file).status().unwrap_err();
        assert_eq!(error.raw_os_error(), Some(6), "{error}");
    }

    #[test]
    fn invalid_backing_lengths_fail_before_loop_control_access() {
        let backing = tempfile::tempfile().unwrap();
        // Invalid inputs only: this test never requests a kernel loop device.
        for length in [0, 1, 4095, 4097, 8191] {
            backing.set_len(length).unwrap();
            for error in [
                LoopDevice::<ReadOnly>::attach(&backing).unwrap_err(),
                LoopDevice::<Writable>::attach(&backing).unwrap_err(),
            ] {
                assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{length}");
                assert!(error.to_string().contains("4096-byte blocks"));
            }
        }
    }

    #[test]
    fn sole_owner_detach_confirms_unbound_loop() {
        let Some(device) = attach_writable() else {
            return;
        };
        let path = node_path(&device);
        device.close().expect("sole owner detach");
        assert_unbound(&path);
    }

    #[test]
    fn shared_probe_lock_is_drained_before_unbound_detach() {
        let Some(device) = attach_writable() else {
            return;
        };
        let path = node_path(&device);
        let (ready_tx, ready_rx) = mpsc::channel();
        let probe_path = path.clone();
        let probe = thread::spawn(move || {
            let held = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&probe_path)
                .unwrap();
            rustix::fs::flock(&held, rustix::fs::FlockOperation::LockShared).unwrap();
            ready_tx.send(()).unwrap();
            thread::sleep(Duration::from_millis(300));
        });
        ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let started = Instant::now();
        device.close().expect("detach after shared probe");
        assert!(
            started.elapsed() >= Duration::from_millis(200),
            "detach returned before the shared probe released the device"
        );
        assert_unbound(&path);
        probe.join().unwrap();
    }

    #[test]
    fn nonlocking_holder_stays_busy() {
        let Some(device) = attach_writable() else {
            return;
        };
        let path = node_path(&device);
        let holder = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let (tx, rx) = mpsc::channel();
        let detach = thread::spawn(move || {
            tx.send(match device.close() {
                Ok(()) => None,
                Err((error, returned)) => {
                    drop(returned);
                    Some(error)
                }
            })
            .unwrap();
        });
        let error = rx
            .recv_timeout(Duration::from_millis(500))
            .expect("detach blocked on a non-locking holder")
            .expect("non-locking holder must keep detach busy");
        assert_eq!(error.kind(), io::ErrorKind::ResourceBusy);
        assert!(error.to_string().contains("live holder"));
        detach.join().unwrap();
        drop(holder);
    }

    #[test]
    fn shared_lock_past_deadline_fails_closed() {
        let Some(device) = attach_writable() else {
            return;
        };
        let path = node_path(&device);
        let (ready_tx, ready_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let probe_path = path;
        let probe = thread::spawn(move || {
            let held = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&probe_path)
                .unwrap();
            rustix::fs::flock(&held, rustix::fs::FlockOperation::LockShared).unwrap();
            ready_tx.send(()).unwrap();
            let _ = release_rx.recv_timeout(PROBE_LOCK_DEADLINE + Duration::from_secs(2));
            drop(held);
        });
        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("probe did not take LOCK_SH");
        let (result_tx, result_rx) = mpsc::channel();
        let detach = thread::spawn(move || {
            let started = Instant::now();
            let outcome = device.close();
            result_tx.send((started.elapsed(), outcome)).unwrap();
        });
        let (elapsed, outcome) = result_rx
            .recv_timeout(PROBE_LOCK_DEADLINE + Duration::from_millis(400))
            .expect("detach hung past the probe-lock deadline");
        let (error, returned) = outcome.expect_err("stuck shared lock must not detach");
        assert_eq!(error.kind(), io::ErrorKind::ResourceBusy);
        assert!(error.to_string().contains("live holder"));
        assert!(
            returned.device.status().is_ok(),
            "deadline failure must leave the loop owner bound"
        );
        assert!(
            elapsed + Duration::from_millis(100) >= PROBE_LOCK_DEADLINE,
            "detach returned before the deadline: {elapsed:?}"
        );
        assert!(
            elapsed < PROBE_LOCK_DEADLINE + Duration::from_millis(400),
            "detach hung past the deadline: {elapsed:?}"
        );
        drop(returned);
        let _ = release_tx.send(());
        detach.join().unwrap();
        probe.join().unwrap();
    }

    #[test]
    fn deadline_expiry_does_not_acquire_once_probe_drops() {
        let Some(device) = attach_writable() else {
            return;
        };
        let path = node_path(&device);
        let (ready_tx, ready_rx) = mpsc::channel();
        let (started_tx, started_rx) = mpsc::channel();
        let probe_path = path;
        let probe = thread::spawn(move || {
            let held = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&probe_path)
                .unwrap();
            rustix::fs::flock(&held, rustix::fs::FlockOperation::LockShared).unwrap();
            ready_tx.send(()).unwrap();
            let started: Instant = started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            // Inside the 20ms poll window after the detach deadline. A loop
            // that flocks again after expiry would take this lock and succeed.
            let release_at = started + PROBE_LOCK_DEADLINE + (PROBE_LOCK_POLL / 2);
            if let Some(wait) = release_at.checked_duration_since(Instant::now()) {
                thread::sleep(wait);
            }
            drop(held);
        });
        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("probe did not take LOCK_SH");
        let (result_tx, result_rx) = mpsc::channel();
        let detach = thread::spawn(move || {
            let started = Instant::now();
            started_tx.send(started).unwrap();
            let outcome = device.close();
            result_tx.send((started.elapsed(), outcome)).unwrap();
        });
        let (elapsed, outcome) = result_rx
            .recv_timeout(PROBE_LOCK_DEADLINE + Duration::from_millis(400))
            .expect("detach hung past the probe-lock deadline");
        let (error, returned) = outcome.expect_err("post-deadline acquire must not detach");
        assert_eq!(error.kind(), io::ErrorKind::ResourceBusy);
        assert!(error.to_string().contains("live holder"));
        assert!(
            returned.device.status().is_ok(),
            "expiry must leave the loop owner bound"
        );
        assert!(
            elapsed < PROBE_LOCK_DEADLINE + Duration::from_millis(400),
            "detach hung past the deadline: {elapsed:?}"
        );
        drop(returned);
        detach.join().unwrap();
        probe.join().unwrap();
    }
}
