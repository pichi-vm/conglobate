// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fd::OwnedFd,
    fs::inotify,
};
use std::{
    io,
    path::{Path, PathBuf},
    time::Instant,
};

/// Fixture-only marker observation. Register before checking the marker so a
/// write between the check and poll cannot be lost. Events request rechecking;
/// neither an event nor elapsed time establishes readiness.
pub(super) struct Readiness {
    events: OwnedFd,
    marker: PathBuf,
}
impl Readiness {
    pub(super) fn new(marker: &Path) -> io::Result<Self> {
        let events = inotify::init(inotify::CreateFlags::CLOEXEC | inotify::CreateFlags::NONBLOCK)?;
        let parent = marker
            .parent()
            .ok_or_else(|| io::Error::other("marker needs parent"))?;
        inotify::add_watch(
            &events,
            parent,
            inotify::WatchFlags::CREATE
                | inotify::WatchFlags::MOVED_TO
                | inotify::WatchFlags::DELETE_SELF
                | inotify::WatchFlags::MOVE_SELF,
        )?;
        Ok(Self {
            events,
            marker: marker.to_owned(),
        })
    }
    pub(super) fn wait_until(
        self,
        deadline: Instant,
        peer_lost: impl Fn() -> io::Result<bool>,
    ) -> io::Result<()> {
        loop {
            if self.marker.try_exists()? {
                return Ok(());
            }
            if peer_lost()? {
                return Err(io::Error::other("helper exited before command write"));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "command start deadline",
                ));
            }
            let timeout = Timespec {
                tv_sec: remaining
                    .as_secs()
                    .try_into()
                    .map_err(|_| io::Error::other("deadline too distant"))?,
                tv_nsec: remaining.subsec_nanos().into(),
            };
            let mut fds = [PollFd::new(&self.events, PollFlags::IN)];
            match poll(&mut fds, Some(&timeout)) {
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
                Ok(_) => {}
            }
            if fds[0]
                .revents()
                .intersects(PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL)
            {
                return Err(io::Error::other("marker event descriptor unavailable"));
            }
            // Drain a bounded batch; unrelated events and overflow never imply
            // success. Recheck state and deadline even under continuous events.
            if fds[0].revents().contains(PollFlags::IN) {
                match rustix::io::read(&self.events, &mut [0u8; 4096]) {
                    Ok(0) => return Err(io::Error::other("marker event stream ended")),
                    Ok(_) | Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn existing_and_queued_markers_are_observed() {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("ready");
        let watch = Readiness::new(&marker).unwrap();
        std::fs::write(&marker, "ready").unwrap();
        watch.wait_until(Instant::now(), || Ok(false)).unwrap();
        Readiness::new(&marker)
            .unwrap()
            .wait_until(Instant::now(), || Ok(false))
            .unwrap();
    }
    #[test]
    fn unrelated_event_does_not_establish_readiness() {
        let directory = tempfile::tempdir().unwrap();
        let watch = Readiness::new(&directory.path().join("ready")).unwrap();
        std::fs::write(directory.path().join("other"), "unrelated").unwrap();
        assert_eq!(
            watch
                .wait_until(Instant::now() + Duration::from_millis(20), || Ok(false))
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
    }
    #[test]
    fn renamed_marker_has_complete_contents() {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("ready");
        let temporary = directory.path().join("preparing");
        let watch = Readiness::new(&marker).unwrap();
        let published = marker.clone();
        let writer = std::thread::spawn(move || {
            std::fs::write(&temporary, "complete").unwrap();
            std::fs::rename(temporary, published).unwrap();
        });
        watch
            .wait_until(Instant::now() + Duration::from_secs(5), || Ok(false))
            .unwrap();
        assert_eq!(std::fs::read_to_string(marker).unwrap(), "complete");
        writer.join().unwrap();
    }
    #[test]
    fn observes_writer_after_registration_without_delay() {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("ready");
        let watch = Readiness::new(&marker).unwrap();
        let writer = std::thread::spawn(move || std::fs::write(marker, "ready").unwrap());
        watch
            .wait_until(Instant::now() + Duration::from_secs(5), || Ok(false))
            .unwrap();
        writer.join().unwrap();
    }
}
