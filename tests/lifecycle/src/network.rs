// SPDX-License-Identifier: Apache-2.0
use conglobate::network::NetworkNamespace;
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::Path,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub(super) struct NetworkProbe;
impl NetworkProbe {
    pub(super) fn capture(
        interface: &std::ffi::OsStr,
        port: &std::ffi::OsStr,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::{
            io::{BufRead, Read},
            process::{Command, Stdio},
        };
        let port: u16 = port.to_str().ok_or("non-UTF8 port")?.parse()?;
        let mut child = Command::new("/usr/bin/tcpdump")
            .args(["-nn", "-l", "-Z", "root", "-i"])
            .arg(interface)
            .args(["-c", "1"])
            .arg(format!(
                "tcp dst port {port} and tcp[tcpflags] & tcp-syn != 0"
            ))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut output = child.stdout.take().ok_or("capture stdout absent")?;
        let mut diagnostics =
            std::io::BufReader::new(child.stderr.take().ok_or("capture stderr absent")?);
        let process = conglobate::process::Process::from(child);
        let mut line = String::new();
        loop {
            line.clear();
            if diagnostics.read_line(&mut line)? == 0 {
                return Err("capture ended before readiness".into());
            }
            eprint!("{line}");
            if line.contains("listening on ") {
                break;
            }
        }
        println!("NETWORK_CAPTURE_READY");
        assert!(
            process
                .wait_until(Instant::now() + Duration::from_secs(120))?
                .status()
                .success()
        );
        let mut packet = String::new();
        output.read_to_string(&mut packet)?;
        eprint!("{packet}");
        assert_eq!(packet.lines().count(), 1);
        assert!(packet.contains(" IP 10.0.2.15."));
        assert!(packet.contains(&format!(" > 10.0.2.2.{port}: Flags [S]")));
        eprintln!("NETWORK_EXTERNAL_IPV4_SOURCE_NAT_PASS");
        Ok(())
    }

    fn ip(arguments: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
        assert!(
            std::process::Command::new("ip")
                .args(arguments)
                .status()?
                .success()
        );
        Ok(())
    }
    pub(super) fn external_input() -> Result<(), Box<dyn std::error::Error>> {
        assert!(Path::new("/conglobate-disposable-validation-vm").exists());
        Self::ip(&["netns", "add", "supplied"])?;
        let pinned = NetworkNamespace::open(Path::new("/run/netns/supplied"))?;
        let original = pinned.metadata()?;
        Self::ip(&["netns", "delete", "supplied"])?;
        Self::ip(&["netns", "add", "supplied"])?;
        let replacement = NetworkNamespace::open(Path::new("/run/netns/supplied"))?;
        assert_ne!(original.ino(), replacement.metadata()?.ino());
        assert_eq!(original.ino(), pinned.metadata()?.ino());
        drop(pinned);
        drop(replacement);
        assert!(Path::new("/run/netns/supplied").exists());
        let retained = NetworkNamespace::open(Path::new("/run/netns/supplied"))?;
        drop(retained);
        Self::ip(&["netns", "delete", "supplied"])?;
        assert!(NetworkNamespace::open(Path::new("/proc/self/ns/net")).is_err());
        assert!(NetworkNamespace::open(Path::new("/missing-network-namespace")).is_err());
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        println!("EXTERNAL_NETWORK_PIN_REPLACEMENT_AND_RELEASE_PASS");
        Ok(())
    }
    pub(super) fn service() -> Result<(), Box<dyn std::error::Error>> {
        Self::command_listener()?
            .join()
            .map_err(|_| "network service panicked")??;
        Ok(())
    }
    fn command_listener() -> std::io::Result<std::thread::JoinHandle<std::io::Result<()>>> {
        use std::io::{BufRead, Write};
        let listener = std::net::TcpListener::bind("10.0.2.15:45457")?;
        let dns = std::net::UdpSocket::bind("10.0.2.15:53")?;
        println!("NETWORK_SERVICE_READY");
        Ok(std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(60);
            let mut queries = 0;
            loop {
                let remaining = deadline
                    .checked_duration_since(Instant::now())
                    .ok_or_else(|| std::io::Error::other("image DNS/TCP deadline"))?;
                let timeout =
                    rustix::event::Timespec::try_from(remaining).map_err(std::io::Error::other)?;
                let mut poll = [
                    rustix::event::PollFd::new(&dns, rustix::event::PollFlags::IN),
                    rustix::event::PollFd::new(&listener, rustix::event::PollFlags::IN),
                ];
                if rustix::event::poll(&mut poll, Some(&timeout))? == 0 {
                    return Err(std::io::Error::other(
                        "image command did not resolve/connect",
                    ));
                }
                if poll[0].revents().contains(rustix::event::PollFlags::IN) {
                    let mut bytes = [0u8; 512];
                    let (length, peer) = dns.recv_from(&mut bytes)?;
                    let name = b"\x05build\x07fixture\0";
                    let end = 12 + name.len() + 4;
                    assert!(length >= end);
                    assert_eq!(&bytes[4..6], &[0, 1]);
                    assert_eq!(&bytes[12..12 + name.len()], name);
                    assert_eq!(&bytes[end - 2..end], &[0, 1]);
                    let ipv4 = bytes[end - 4..end - 2] == [0, 1];
                    assert!(ipv4 || bytes[end - 4..end - 2] == [0, 28]);
                    let mut response = vec![
                        bytes[0],
                        bytes[1],
                        0x81,
                        0x80,
                        0,
                        1,
                        0,
                        u8::from(ipv4),
                        0,
                        0,
                        0,
                        0,
                    ];
                    response.extend_from_slice(&bytes[12..end]);
                    if ipv4 {
                        response.extend_from_slice(&[
                            0xc0, 0x0c, 0, 1, 0, 1, 0, 0, 0, 1, 0, 4, 10, 0, 2, 15,
                        ]);
                    }
                    dns.send_to(&response, peer)?;
                    queries += 1;
                    assert!(queries <= 16);
                }
                if poll[1].revents().contains(rustix::event::PollFlags::IN) {
                    let (mut stream, _) = listener.accept()?;
                    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
                    let mut request = String::new();
                    std::io::BufReader::new(&stream).read_line(&mut request)?;
                    assert_eq!(request, "request\n");
                    assert!(queries > 0);
                    stream.write_all(b"reply\n")?;
                    eprintln!("NETWORK_RESTRICTED_IMAGE_DNS_AND_TCP_PASS");
                    return Ok(());
                }
            }
        }))
    }
}
