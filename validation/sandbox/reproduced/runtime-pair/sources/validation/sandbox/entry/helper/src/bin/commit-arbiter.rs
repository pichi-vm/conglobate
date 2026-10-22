//! Diagnostic coordinator for already completed, owned fixture artifacts.
#![forbid(unsafe_code)]
use nix::fcntl::{open, renameat2, Flock, FlockArg, OFlag, RenameFlags};
use nix::sys::stat::Mode;
use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Write},
    os::unix::{fs::OpenOptionsExt, net::{UnixListener, UnixStream}},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase { Building, Prepared, Reserved, Committing, Cancelled, Committed, Collision, Failed, VisibleError }

fn request(mut socket: UnixStream, parent: Arc<File>, state: Arc<Mutex<Phase>>) -> std::io::Result<()> {
    let mut line = String::new();
    BufReader::new(socket.try_clone()?).take(64).read_line(&mut line)?;
    if !line.ends_with('\n') { return Err(std::io::Error::other("bounded request requires newline")); }
    let operation = line.trim();
    let reply = match operation {
        "cancel" => {
            let mut phase = state.lock().unwrap();
            match *phase {
                Phase::Building | Phase::Prepared | Phase::Cancelled => { *phase = Phase::Cancelled; "Accepted".to_owned() }
                Phase::Reserved | Phase::Committing => "Pending".to_owned(),
                terminal => format!("{terminal:?}"),
            }
        }
        "prepare" => {
            // Trusted coordinator only: this request asserts a completed barrier;
            // it cannot independently establish filesystem/process quiescence.
            let mut phase = state.lock().unwrap();
            if *phase == Phase::Building { *phase = Phase::Prepared; }
            format!("{phase:?}")
        }
        "reserve" => {
            let mut phase = state.lock().unwrap();
            if *phase == Phase::Prepared { *phase = Phase::Reserved; }
            format!("{phase:?}")
        }
        "status" => format!("{:?}", *state.lock().unwrap()),
        "commit" | "commit-no-reply" => {
            let admitted = {
                let mut phase = state.lock().unwrap();
                if *phase == Phase::Reserved { *phase = Phase::Committing; true } else { false }
            };
            if admitted {
                // State is Committing before releasing the lock for filesystem
                // work, so cancellation cannot promise absence of output.
                let outcome = match renameat2(&*parent, "published", &*parent, "result", RenameFlags::RENAME_NOREPLACE) {
                    Ok(()) => if parent.sync_all().is_ok() { Phase::Committed } else { Phase::VisibleError },
                    Err(nix::errno::Errno::EEXIST) => Phase::Collision,
                    Err(_) => Phase::Failed,
                };
                *state.lock().unwrap() = outcome;
                if operation == "commit-no-reply" {
                    // Bounded observer kills this owned process after checking
                    // status through another private control connection.
                    loop { thread::park(); }
                }
            }
            format!("{:?}", *state.lock().unwrap())
        }
        _ => "InvalidRequest".to_owned(),
    };
    writeln!(socket, "{reply}")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(args.next().ok_or("owned artifact directory required")?);
    let mode = args.next();
    let lease_gate = mode.as_deref() == Some("--lease-gate");
    let parent = match mode.as_deref() {
        None | Some("--lease-gate") | Some("--await-completion") => File::open(&path)?,
        Some("--path-parent") => {
            // Diagnostic only: O_PATH is valid for renameat2's directory
            // arguments but fsync must reject this descriptor with EBADF.
            // Exercise a real post-rename syscall error, not simulated EIO.
            let file = File::from(open(&path, OFlag::O_PATH | OFlag::O_DIRECTORY | OFlag::O_CLOEXEC, Mode::empty())?);
            assert_eq!(file.sync_all().unwrap_err().raw_os_error(), Some(nix::errno::Errno::EBADF as i32));
            file
        }
        Some(_) => return Err("unknown diagnostic mode".into()),
    };
    assert!(args.next().is_none());
    // Stable protected inode. Never unlink/recreate it to retry an attempt;
    // no lease descriptor is handed to recipe code or another process.
    let lease = File::options().read(true).write(true).create_new(true)
        .mode(0o600).open(path.join("recovery.lease.new"))?;
    let _lease = Flock::lock(lease, FlockArg::LockExclusiveNonblock)
        .map_err(|(_, error)| error)?;
    if lease_gate {
        println!("LEASE_LOCKED_PRIVATE");
        std::io::stdout().flush()?;
        let mut release = String::new();
        std::io::stdin().read_line(&mut release)?;
        assert_eq!(release, "publish\n");
    }
    // Recovery sees either no lease (Unknown) or this already-locked inode.
    // Never replace an existing lease, including an unlocked old attempt.
    renameat2(&parent, "recovery.lease.new", &parent, "recovery.lease", RenameFlags::RENAME_NOREPLACE)?;
    if lease_gate {
        println!("LEASE_PUBLISHED_LOCKED");
        std::io::stdout().flush()?;
    }
    let parent = Arc::new(parent);
    let listener = UnixListener::bind(path.join("control.sock"))?;
    let state = Arc::new(Mutex::new(if mode.as_deref() == Some("--await-completion") { Phase::Building } else { Phase::Prepared }));
    for incoming in listener.incoming() {
        let socket = incoming?;
        let (parent, state) = (parent.clone(), state.clone());
        thread::spawn(move || { request(socket, parent, state).expect("trusted fixture request failed"); });
    }
    Ok(())
}
