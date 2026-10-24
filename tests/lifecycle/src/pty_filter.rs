// SPDX-License-Identifier: Apache-2.0
use conglobate::process::CommandSyscalls;
use nix::sys::termios::{SetArg, cfmakeraw, tcgetattr, tcsetattr};
use rustix::{mount, pty};
use std::{
    fs,
    io::{Read, Write},
};

/// Exercise Nix's PTY log sequence only inside an owned disposable VM.
#[derive(Debug)]
pub(super) struct PtyFilter;
impl PtyFilter {
    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all("/dev/pts")?;
        mount::mount(
            "devpts",
            "/dev/pts",
            "devpts",
            mount::MountFlags::NOSUID | mount::MountFlags::NOEXEC,
            c"newinstance,ptmxmode=0666,mode=0600",
        )?;
        fs::remove_file("/dev/ptmx")?;
        std::os::unix::fs::symlink("pts/ptmx", "/dev/ptmx")?;
        CommandSyscalls::compile()?.apply()?;
        let master = pty::openpt(pty::OpenptFlags::RDWR | pty::OpenptFlags::NOCTTY)?;
        let name = pty::ptsname(&master, Vec::new())?;
        pty::unlockpt(&master)?;
        let mut slave = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(name.to_str()?)?;
        let mut attributes = tcgetattr(&slave)?;
        cfmakeraw(&mut attributes);
        tcsetattr(&slave, SetArg::TCSANOW, &attributes)
            .map_err(|error| std::io::Error::other(format!("set raw terminal mode: {error}")))?;
        // A different, harmless terminal mutation remains outside the allowlist.
        assert_eq!(
            rustix::termios::tcsetwinsize(
                &slave,
                rustix::termios::Winsize {
                    ws_row: 24,
                    ws_col: 80,
                    ws_xpixel: 0,
                    ws_ypixel: 0,
                }
            ),
            Err(rustix::io::Errno::PERM)
        );
        slave.write_all(b"nix-log\n")?;
        let mut received = [0; 8];
        fs::File::from(master).read_exact(&mut received)?;
        assert_eq!(&received, b"nix-log\n");
        println!("PRIVATE_PTY_FILTER_PASS");
        Ok(())
    }
}
