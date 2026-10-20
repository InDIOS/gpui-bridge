//! Runs the wire-level transcripts in `conformance/` (format: PROTOCOL.md section 9).

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;

/// Every field of `expected` must be equal in `actual`; extra fields are allowed.
fn matches(expected: &Value, actual: &Value) -> bool {
    match (expected, actual) {
        (Value::Object(expected), Value::Object(actual)) => expected
            .iter()
            .all(|(key, value)| actual.get(key) == Some(value)),
        _ => expected == actual,
    }
}

/// Plays one transcript; returns a description of the first mismatch.
fn run(path: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(path).unwrap();
    let mut sent = Vec::new();
    let mut expected = Vec::new();
    for (number, line) in text.lines().enumerate() {
        match line.split_at_checked(2) {
            Some(("> ", message)) => sent.push(message),
            Some(("< ", message)) => expected.push(
                serde_json::from_str::<Value>(message)
                    .map_err(|err| format!("line {}: {err}", number + 1))?,
            ),
            _ if line.is_empty() || line.starts_with('#') => {}
            _ => return Err(format!("line {}: expected '> ', '< ' or '#'", number + 1)),
        }
    }

    let mut child = Command::new(env!("CARGO_BIN_EXE_gpui-bridge-host"))
        .arg("--no-window")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to start host");
    // shortcut: all input is written before reading, fine while transcripts stay far below the pipe buffer.
    let mut stdin = child.stdin.take().unwrap();
    for line in &sent {
        // The host may exit mid-transcript (protocol_mismatch); the output check reports it.
        let _ = writeln!(stdin, "{line}");
    }
    drop(stdin);

    let actual: Vec<Value> = BufReader::new(child.stdout.take().unwrap())
        .lines()
        .map(|line| {
            let line = line.unwrap();
            serde_json::from_str(&line)
                .unwrap_or_else(|_| Value::String(format!("non-JSON stdout: {line}")))
        })
        .collect();
    let status = child.wait().unwrap();

    for (index, want) in expected.iter().enumerate() {
        match actual.get(index) {
            Some(got) if matches(want, got) => {}
            got => {
                return Err(format!(
                    "message {}: expected {want}, got {got:?}\nall output: {actual:#?}",
                    index + 1
                ));
            }
        }
    }
    if actual.len() > expected.len() {
        return Err(format!(
            "unexpected extra output: {:#?}",
            &actual[expected.len()..]
        ));
    }
    if !status.success() {
        return Err(format!("host exited with {status}"));
    }
    Ok(())
}

#[test]
fn conformance_transcripts() {
    // packages/host -> repository root -> conformance/
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .expect("conformance/ not found")
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "txt"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no transcripts in {}", dir.display());

    let failures: Vec<String> = paths
        .iter()
        .filter_map(|path| {
            run(path)
                .err()
                .map(|err| format!("{}: {err}", path.file_name().unwrap().display()))
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} transcripts failed:\n{}",
        failures.len(),
        paths.len(),
        failures.join("\n\n")
    );
}
