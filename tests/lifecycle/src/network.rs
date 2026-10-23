// SPDX-License-Identifier: Apache-2.0
use conglobate::{execution::HelperProgram, network::FirewallTable, step::SetupBudget};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
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

    pub(super) fn allocation_lock_child(contended: bool) -> Result<(), Box<dyn std::error::Error>> {
        let namespace = fs::File::open("/proc/self/ns/net")?;
        let result = rustix::fs::flock(
            &namespace,
            rustix::fs::FlockOperation::NonBlockingLockExclusive,
        );
        if contended {
            assert_eq!(result.unwrap_err(), rustix::io::Errno::WOULDBLOCK);
        } else {
            result?;
        }
        Ok(())
    }
    fn allocation_lock_probe() -> Result<(), Box<dyn std::error::Error>> {
        let namespace = fs::File::open("/proc/self/ns/net")?;
        rustix::fs::flock(
            &namespace,
            rustix::fs::FlockOperation::NonBlockingLockExclusive,
        )?;
        let other = fs::File::open("/proc/self/ns/net")?;
        assert_eq!(
            rustix::fs::flock(&other, rustix::fs::FlockOperation::NonBlockingLockExclusive)
                .unwrap_err(),
            rustix::io::Errno::WOULDBLOCK
        );
        assert!(
            std::process::Command::new(std::env::current_exe()?)
                .arg("network-allocation-lock-child")
                .arg("true")
                .status()?
                .success()
        );
        for (namespace, contended) in [("--mount", "true"), ("--net", "false")] {
            assert!(
                std::process::Command::new("unshare")
                    .arg(namespace)
                    .arg(std::env::current_exe()?)
                    .arg("network-allocation-lock-child")
                    .arg(contended)
                    .status()?
                    .success()
            );
        }
        drop(namespace);
        rustix::fs::flock(&other, rustix::fs::FlockOperation::NonBlockingLockExclusive)?;
        println!("NETWORK_NAMESPACE_FLOCK_EXCLUSION_AND_RELEASE_PASS");
        Ok(())
    }
    pub(super) fn composite_failures() -> Result<(), Box<dyn std::error::Error>> {
        use conglobate::{
            network::{CommandNetwork, ResolverInput},
            storage::Workspace,
        };
        Self::allocation_lock_probe()?;
        let workspace = Workspace::create()?;
        let program = HelperProgram::current()?;
        let parent = Path::new("/work/network-composite");
        fs::create_dir(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        let resolver = parent.join("resolver-input");
        fs::write(&resolver, b"nameserver 192.0.2.53\n")?;
        let resolver = ResolverInput::capture(&resolver)?;
        fs::remove_file(parent.join("resolver-input"))?;
        // Fixture-only configuration in the NIC-less disposable VM.
        fs::write("/proc/sys/net/ipv4/ip_forward", "1")?;
        fs::write("/proc/sys/net/ipv6/conf/all/forwarding", "1")?;
        let held = fs::File::open("/proc/self/ns/net")?;
        rustix::fs::flock(&held, rustix::fs::FlockOperation::NonBlockingLockExclusive)?;
        let failure = CommandNetwork::prepare(
            &resolver,
            &workspace,
            parent,
            &program,
            &SetupBudget::begin(),
        )
        .unwrap_err();
        assert!(failure.to_string().contains("network allocation lease"));
        failure.cleanup()?;
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        assert!(!Path::new("/sys/class/net/cgb0h").exists());
        drop(held);
        println!("NETWORK_PRODUCTION_ALLOCATION_CONTENTION_BEFORE_MUTATION_PASS");
        let failure = CommandNetwork::prepare(
            &resolver,
            &workspace,
            &parent.join("missing"),
            &program,
            &SetupBudget::begin(),
        )
        .unwrap_err();
        failure.cleanup()?;
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        assert!(!Path::new("/sys/class/net/cgb0h").exists());
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        println!("NETWORK_COMPOSITE_AFTER_NAMESPACE_FAILURE_CLEANUP_PASS");

        let foreign =
            FirewallTable::create("conglobate_cgb0h", Instant::now() + Duration::from_secs(10))?;
        let failure = CommandNetwork::prepare(
            &resolver,
            &workspace,
            parent,
            &program,
            &SetupBudget::begin(),
        )
        .unwrap_err();
        assert!(failure.to_string().contains("NAT installation"));
        failure.cleanup()?;
        assert!(
            std::process::Command::new("nft")
                .args(["list", "table", "inet", "conglobate_cgb0h"])
                .output()?
                .status
                .success()
        );
        assert!(!Path::new("/sys/class/net/cgb0h").exists());
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        foreign.close(Instant::now() + Duration::from_secs(10))?;
        println!("NETWORK_COMPOSITE_NAT_FAILURE_PRESERVES_FOREIGN_TABLE_PASS");

        let network = CommandNetwork::prepare(
            &resolver,
            &workspace,
            parent,
            &program,
            &SetupBudget::begin(),
        )?;
        let second = CommandNetwork::prepare(
            &resolver,
            &workspace,
            parent,
            &program,
            &SetupBudget::begin(),
        )?;
        assert!(Path::new("/sys/class/net/cgb1h").exists());
        let second_address = std::net::UdpSocket::bind("10.0.0.5:0")?;
        drop(second_address);
        second.close(Instant::now() + Duration::from_secs(10))?;
        assert!(!Path::new("/sys/class/net/cgb1h").exists());
        assert!(Path::new("/sys/class/net/cgb0h").exists());
        println!("NETWORK_PRODUCTION_ALLOCATION_RELEASE_AND_DISTINCT_LIVE_PREFIX_PASS");
        let mut failure = network.close(Instant::now()).unwrap_err();
        for _ in 0..2 {
            assert!(
                failure
                    .diagnostics()
                    .to_string()
                    .contains("deadline expired")
            );
            assert!(Path::new("/sys/class/net/cgb0h").exists());
            assert!(
                std::process::Command::new("nft")
                    .args(["list", "table", "inet", "conglobate_cgb0h"])
                    .output()?
                    .status
                    .success()
            );
            assert_eq!(fs::read_dir(parent)?.count(), 1);
            failure = failure.retry().unwrap_err();
        }
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        println!("NETWORK_COMPOSITE_EXPIRED_TEARDOWN_RETRY_RETAINS_OWNER_PASS");
        // This is deliberately the final probe. No successful cleanup is
        // claimed: the VM controller disposes of retained resources on exit.
        drop(failure);
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
