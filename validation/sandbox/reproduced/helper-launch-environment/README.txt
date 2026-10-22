Trusted helper environment constructed before exec
=================================================
Actual src/execute.rs change, not replacement of the legacy executor.
Step::command now clears the inherited environment and sets PATH, HOME and LC_ALL
before spawning trusted __step/__init helpers. Both call sites use this function.
The recipe shell's existing later environment clearing remains separate.

A nonprivileged subprocess regression uses an owned temporary Bash startup hook
that writes only an owned marker. Direct Bash invocation is the positive control.
An isolated test subprocess inherits that hook and invokes the actual private
Step::command constructor with an owned executable probe. Before the change the
helper probe fails its environment checks; afterward it succeeds with the explicit
environment and no marker. No namespace, mount, credential change, external file,
LD_PRELOAD payload or privileged image code is involved in this host test.

All13 library tests pass, including this regression; strict library Clippy and
format checks pass. Sources, command, initial failure and final logs are retained.
Reproduce: cargo test --locked --lib
           cargo clippy --locked --lib -- -D warnings
           cargo fmt --check

This demonstrates child-exec environment hygiene, not protection before the
original conglobate process loads, complete dynamic loader/configuration binding,
trusted runtime immutability, or final namespace/FUSE executor adoption. The
current executor retains other documented gaps; no root-namespace tests were
rerun here. The binary's executable hash is attribution, not self-attestation.
