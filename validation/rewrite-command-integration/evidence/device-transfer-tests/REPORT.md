# Device transfer negative coverage

Maintained tests now exercise the actual private sender/receiver routines without
changing process stdin, mounting filesystems or modifying privileges. An ordinary
unprivileged tmpfs directory supplies the positive transport fixture; it does not
qualify device construction or mount attachment.

Coverage: empty/short/wrong-version/wrong-opcode/trailing/truncated payloads;
zero, duplicate and truncated ancillary descriptor counts; non-directory and
non-tmpfs directory descriptors; nonblocking receipt without an offer; and valid
ownership transfer with CLOEXEC after the sender and socket are gone. Rejection
checks use socket-peer EOF to prove rights are released, avoiding process-wide FD
counts that race other tests. The existing disposable-VM v29 remains the latest
actual device/mount/command evidence.

The initial test draft failed to compile because Stdio cannot convert back into
OwnedFd. The production methods now delegate privately to into_channel/receive,
which tests can call directly. No public API or wire contract changed. Full
cargo test passed; a final non-tmpfs case was then added and the focused tests
passed again. Clippy with warnings denied and formatting pass. Exact source and
logs are preserved here. No milestone acceptance or full sandbox claim follows.
