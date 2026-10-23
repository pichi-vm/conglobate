// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::{
    anchor::RecoveryDirectory,
    execution::HelperProgram,
    isolation::{CommandDevices, CommandSetup, IdRange, Mapped, UserNamespace},
    network::{
        ConfiguredVeth, InterfaceName, NatPolicy, NetworkAddress, NetworkNamespace, VethPair,
    },
    process::Process,
    step::SetupBudget,
};
use std::{
    fs,
    io::{self, Read, Write},
    net::{IpAddr, SocketAddr, TcpListener, TcpStream, UdpSocket},
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(crate) struct ForwardingProbe;
impl ForwardingProbe {
    fn listener(address: &str) -> io::Result<TcpListener> {
        let address: SocketAddr = address.parse().map_err(io::Error::other)?;
        let family = if address.is_ipv4() {
            rustix::net::AddressFamily::INET
        } else {
            rustix::net::AddressFamily::INET6
        };
        let socket = rustix::net::socket_with(
            family,
            rustix::net::SocketType::STREAM,
            rustix::net::SocketFlags::CLOEXEC,
            Some(rustix::net::ipproto::TCP),
        )?;
        rustix::net::sockopt::set_socket_reuseaddr(&socket, true)?;
        rustix::net::bind(&socket, &address)?;
        rustix::net::listen(&socket, 4)?;
        Ok(TcpListener::from(socket))
    }
    pub(crate) fn helper(server: bool, nat: bool) -> Result<(), Box<dyn std::error::Error>> {
        let (_scope, _devices) =
            CommandSetup::from_input()?.enter_until(Instant::now() + Duration::from_secs(10))?;
        if server {
            let listeners = ["198.18.0.2:45454", "[fd42:55::2]:45454"]
                .into_iter()
                .map(UdpSocket::bind)
                .collect::<Result<Vec<_>, _>>()?;
            let tcp = [
                Self::listener("198.18.0.2:45456")?,
                Self::listener("[fd42:55::2]:45456")?,
            ];
            io::stdout().write_all(b"R")?;
            io::stdout().flush()?;
            for (socket, expected, target) in [
                (
                    &listeners[0],
                    if nat { "198.18.0.1" } else { "10.44.0.2" },
                    "10.44.0.2:45455",
                ),
                (
                    &listeners[1],
                    if nat { "fd42:55::1" } else { "fd42:44::2" },
                    "[fd42:44::2]:45455",
                ),
            ] {
                socket.set_read_timeout(Some(Duration::from_secs(5)))?;
                let mut bytes = [0; 32];
                let (count, source) = socket.recv_from(&mut bytes)?;
                assert_eq!(&bytes[..count], b"upstream");
                assert_eq!(source.ip(), expected.parse::<IpAddr>()?);
                socket.send_to(b"reply", source)?;
                socket.send_to(b"unsolicited", target)?;
            }
            for (listener, expected, incoming) in [
                (
                    &tcp[0],
                    if nat { "198.18.0.1" } else { "10.44.0.2" },
                    "10.44.0.2:45458",
                ),
                (
                    &tcp[1],
                    if nat { "fd42:55::1" } else { "fd42:44::2" },
                    "[fd42:44::2]:45458",
                ),
            ] {
                let (mut stream, source) = listener.accept()?;
                assert_eq!(source.ip(), expected.parse::<IpAddr>()?);
                stream.set_read_timeout(Some(Duration::from_secs(3)))?;
                stream.set_write_timeout(Some(Duration::from_secs(3)))?;
                let mut bytes = [0; 3];
                stream.read_exact(&mut bytes)?;
                assert_eq!(&bytes, b"tcp");
                // The client binds its listener before sending this request.
                // An outbound established stream does not authorize a new
                // inbound connection to a different port.
                let incoming =
                    TcpStream::connect_timeout(&incoming.parse()?, Duration::from_millis(250));
                if nat {
                    assert_eq!(incoming.unwrap_err().kind(), io::ErrorKind::TimedOut);
                } else {
                    let mut incoming = incoming?;
                    incoming.set_write_timeout(Some(Duration::from_secs(3)))?;
                    incoming.write_all(b"inbound")?;
                }
                stream.write_all(b"reply")?;
            }
        } else {
            for (bind, incoming, upstream) in [
                (
                    if nat {
                        "0.0.0.0:45000"
                    } else {
                        "0.0.0.0:45001"
                    },
                    "0.0.0.0:45455",
                    "198.18.0.2:45454",
                ),
                (
                    if nat { "[::]:45000" } else { "[::]:45001" },
                    "[::]:45455",
                    "[fd42:55::2]:45454",
                ),
            ] {
                let incoming = UdpSocket::bind(incoming)?;
                incoming.set_read_timeout(Some(Duration::from_secs(1)))?;
                let socket = UdpSocket::bind(bind)?;
                socket.set_read_timeout(Some(Duration::from_secs(5)))?;
                socket.connect(upstream)?;
                socket.send(b"upstream")?;
                let mut bytes = [0; 32];
                let count = socket.recv(&mut bytes)?;
                assert_eq!(&bytes[..count], b"reply");
                let result = incoming.recv(&mut bytes);
                if nat {
                    assert!(matches!(
                        result.unwrap_err().kind(),
                        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                    ));
                } else {
                    assert_eq!(&bytes[..result?], b"unsolicited");
                }
            }
            for (target, incoming) in [
                ("198.18.0.2:45456", "10.44.0.2:45458"),
                ("[fd42:55::2]:45456", "[fd42:44::2]:45458"),
            ] {
                let incoming = Self::listener(incoming)?;
                incoming.set_nonblocking(true)?;
                let mut stream =
                    TcpStream::connect_timeout(&target.parse()?, Duration::from_secs(3))?;
                stream.set_read_timeout(Some(Duration::from_secs(3)))?;
                stream.set_write_timeout(Some(Duration::from_secs(3)))?;
                stream.write_all(b"tcp")?;
                let mut bytes = [0; 5];
                stream.read_exact(&mut bytes)?;
                assert_eq!(&bytes, b"reply");
                let received = incoming.accept();
                if nat {
                    assert_eq!(received.unwrap_err().kind(), io::ErrorKind::WouldBlock);
                } else {
                    let (mut received, _) = received?;
                    received.set_read_timeout(Some(Duration::from_secs(3)))?;
                    let mut bytes = [0; 7];
                    received.read_exact(&mut bytes)?;
                    assert_eq!(&bytes, b"inbound");
                }
            }
        }
        Ok(())
    }
    fn link(
        parent: &Path,
        program: &HelperProgram,
        names: (&str, &str),
        hosts: [&str; 2],
        peers: [&str; 2],
    ) -> Result<ConfiguredVeth, Box<dyn std::error::Error>> {
        let pair = VethPair::create(
            parent,
            InterfaceName::try_from(names.0)?,
            InterfaceName::try_from(names.1)?,
            Instant::now() + Duration::from_secs(10),
        )?;
        let namespace = NetworkNamespace::create(program, SetupBudget::begin().cutoff()?)?;
        let connected = pair.connect(namespace, Instant::now() + Duration::from_secs(10))?;
        let hosts: Vec<IpAddr> = hosts
            .into_iter()
            .map(str::parse)
            .collect::<Result<_, _>>()?;
        let peers: Vec<IpAddr> = peers
            .into_iter()
            .map(str::parse)
            .collect::<Result<_, _>>()?;
        Ok(connected.configure(
            vec![
                NetworkAddress::new(hosts[0], 30, None)?,
                NetworkAddress::new(hosts[1], 64, None)?,
            ],
            vec![
                NetworkAddress::new(peers[0], 30, Some(hosts[0]))?,
                NetworkAddress::new(peers[1], 64, Some(hosts[1]))?,
            ],
            program,
            SetupBudget::begin().cutoff()?,
        )?)
    }
    fn exchange(
        user: &Path,
        upstream: &NetworkNamespace,
        command: &NetworkNamespace,
        nat: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let setup = CommandSetup::new(
            fs::File::open(user)?,
            CommandDevices::create(100000, 100000)?,
        )?
        .with_network(upstream)?;
        let mut server = Command::new(std::env::current_exe()?)
            .arg("forward-helper")
            .arg("true")
            .arg(nat.to_string())
            .stdin(setup.into_input()?)
            .stdout(Stdio::piped())
            .spawn()?;
        let mut ready = server.stdout.take().ok_or("missing readiness pipe")?;
        let server = Process::from(server);
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let timeout = rustix::event::Timespec::try_from(
                deadline
                    .checked_duration_since(Instant::now())
                    .ok_or("upstream readiness deadline")?,
            )?;
            let mut fds = [rustix::event::PollFd::new(
                &ready,
                rustix::event::PollFlags::IN,
            )];
            match rustix::event::poll(&mut fds, Some(&timeout)) {
                Err(rustix::io::Errno::INTR) => continue,
                result => {
                    result?;
                    if fds[0].revents().is_empty() {
                        return Err("upstream readiness expired".into());
                    }
                    break;
                }
            }
        }
        let mut byte = [0];
        ready.read_exact(&mut byte)?;
        assert_eq!(byte, *b"R");
        drop(ready);
        let setup = CommandSetup::new(
            fs::File::open(user)?,
            CommandDevices::create(100000, 100000)?,
        )?
        .with_network(command)?;
        let client = Command::new(std::env::current_exe()?)
            .arg("forward-helper")
            .arg("false")
            .arg(nat.to_string())
            .stdin(setup.into_input()?)
            .spawn()?;
        assert!(
            Process::from(client)
                .wait_until(deadline)?
                .status()
                .success()
        );
        assert!(server.wait_until(deadline)?.status().success());
        Ok(())
    }
    pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
        // Fixture-only environment setup, inside the guarded disposable VM.
        // There is no host NIC: both command and upstream links live in this VM.
        fs::write("/proc/sys/net/ipv4/ip_forward", "1")?;
        fs::write("/proc/sys/net/ipv6/conf/all/forwarding", "1")?;
        let parent = Path::new("/work/forward-control");
        fs::create_dir(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        let pins = parent.join("pins");
        fs::create_dir(&pins)?;
        fs::set_permissions(&pins, fs::Permissions::from_mode(0o700))?;
        let recovery = RecoveryDirectory::open(&pins)?;
        let program = HelperProgram::current()?;
        let cutoff = SetupBudget::begin().cutoff()?;
        let user = UserNamespace::<Mapped>::create(
            program.namespace_creator(cutoff)?,
            IdRange::new(100000, 65536)?,
            IdRange::new(100000, 65536)?,
            cutoff.deadline()?,
        )?
        .pin(&recovery)?;
        let upstream = Self::link(
            parent,
            &program,
            ("cg-up", "cg-external"),
            ["198.18.0.1", "fd42:55::1"],
            ["198.18.0.2", "fd42:55::2"],
        )?;
        let command = Self::link(
            parent,
            &program,
            ("cg-host", "cg-peer"),
            ["10.44.0.1", "fd42:44::1"],
            ["10.44.0.2", "fd42:44::2"],
        )?;
        let policy = NatPolicy::install(command, Instant::now() + Duration::from_secs(10))?;
        Self::exchange(
            &pins.join(user.record().name()),
            upstream.namespace(),
            policy.namespace(),
            true,
        )?;
        println!("NETWORK_FORWARDED_DUAL_STACK_NAT_AND_REPLY_ONLY_PASS");
        println!("NETWORK_FORWARDED_DUAL_STACK_TCP_PASS");
        let command = policy
            .close(Instant::now() + Duration::from_secs(10))
            .expect("policy cleanup");
        Self::exchange(
            &pins.join(user.record().name()),
            upstream.namespace(),
            command.namespace(),
            false,
        )?;
        println!("NETWORK_FORWARDED_WITHOUT_POLICY_POSITIVE_CONTROL_PASS");
        println!("NETWORK_FORWARDED_UNSOLICITED_TCP_DENIAL_AND_POSITIVE_CONTROL_PASS");
        command.close()?;
        upstream.close()?;
        user.release()?;
        drop(recovery);
        fs::remove_dir(&pins)?;
        fs::remove_dir(parent)?;
        for name in ["cg-host", "cg-peer", "cg-up", "cg-external"] {
            assert!(!Path::new("/sys/class/net").join(name).exists());
        }
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        println!("NETWORK_FORWARDED_TOPOLOGY_TEARDOWN_PASS");
        Ok(())
    }
}
