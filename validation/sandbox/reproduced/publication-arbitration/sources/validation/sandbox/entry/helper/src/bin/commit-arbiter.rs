//! Diagnostic coordinator for already completed, owned fixture artifacts.
#![forbid(unsafe_code)]
use nix::fcntl::{renameat2, RenameFlags};
use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase { Prepared, Reserved, Committing, Cancelled, Committed, Collision, Failed, VisibleError }

fn request(mut socket: UnixStream, parent: Arc<File>, state: Arc<Mutex<Phase>>) -> std::io::Result<()> {
    let mut line = String::new();
    BufReader::new(socket.try_clone()?).take(64).read_line(&mut line)?;
    if !line.ends_with('\n') { return Err(std::io::Error::other("bounded request requires newline")); }
    let operation = line.trim();
    let reply = match operation {
        "cancel" => {
            let mut phase = state.lock().unwrap();
            match *phase {
                Phase::Prepared | Phase::Cancelled => { *phase = Phase::Cancelled; "Accepted".to_owned() }
                Phase::Reserved | Phase::Committing => "Pending".to_owned(),
                terminal => format!("{terminal:?}"),
            }
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
    let path = PathBuf::from(std::env::args().nth(1).ok_or("owned artifact directory required")?);
    let parent = Arc::new(File::open(&path)?);
    let listener = UnixListener::bind(path.join("control.sock"))?;
    let state = Arc::new(Mutex::new(Phase::Prepared));
    for incoming in listener.incoming() {
        let socket = incoming?;
        let (parent, state) = (parent.clone(), state.clone());
        thread::spawn(move || { request(socket, parent, state).expect("trusted fixture request failed"); });
    }
    Ok(())
}
