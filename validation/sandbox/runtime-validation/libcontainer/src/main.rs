#![forbid(unsafe_code)]
use libcontainer::container::builder::ContainerBuilder;
use libcontainer::syscall::syscall::SyscallType;
use nix::sys::wait::{WaitStatus, waitpid};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Disposable-guest probe only. A single-threaded process owns creation,
    // waiting and teardown; no interactive terminal or daemon protocol.
    let args: Vec<_> = std::env::args().collect();
    let bundle = &args[1];
    let id = args[2].clone();
    let fds: i32 = args[3].parse()?;
    nix::sys::prctl::set_child_subreaper(true)?;
    let (mut container, _) = ContainerBuilder::new(id, SyscallType::default())
        .with_root_path("/run/libcontainer")?
        .with_preserved_fds(fds)
        .as_init(bundle)
        .with_systemd(false)
        .with_detach(false)
        .build()?;
    container.start()?;
    let pid = container.pid().ok_or("missing container pid")?;
    let code = loop {
        match waitpid(nix::unistd::Pid::from_raw(pid.as_raw()), None)? {
            WaitStatus::Exited(_, code) => break code,
            WaitStatus::Signaled(_, signal, _) => break 128 + signal as i32,
            _ => {}
        }
    };
    container.delete(true)?;
    std::process::exit(code);
}
