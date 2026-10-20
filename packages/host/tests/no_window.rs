//! End-to-end checks of the binary in `--no-window` mode that a transcript cannot express.
//! Request/response behavior lives in `conformance/` (see `conformance.rs`).

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn protocol_mismatch_exits_without_stdin_closing() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gpui-bridge-host"))
        .arg("--no-window")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to start host");
    let stdin = child.stdin.as_mut().unwrap();
    writeln!(stdin, r#"{{"op":"hello","protocol":99}}"#).unwrap();
    stdin.flush().unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert!(
        line.contains("protocol_mismatch"),
        "unexpected answer: {line}"
    );

    // stdin stays open: the host must exit on its own.
    let deadline = Instant::now() + Duration::from_secs(10);
    while child.try_wait().unwrap().is_none() {
        assert!(
            Instant::now() < deadline,
            "host did not exit after protocol_mismatch"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}
