//! JSON Lines transport. This is the only module allowed to write to stdout.

use std::io::{BufRead, Write};

use futures::channel::mpsc::{UnboundedReceiver, unbounded};
use serde::Serialize;

/// Reads stdin line by line on a dedicated thread (GPUI owns the main thread).
/// The receiver ends when stdin reaches EOF or fails.
pub fn spawn_stdin_reader() -> UnboundedReceiver<String> {
    let (tx, rx) = unbounded();
    std::thread::Builder::new()
        .name("stdin-reader".into())
        .spawn(move || {
            for line in std::io::stdin().lock().lines() {
                match line {
                    Ok(line) if line.trim().is_empty() => continue,
                    Ok(line) => {
                        if tx.unbounded_send(line).is_err() {
                            break;
                        }
                    }
                    Err(err) => {
                        log::error!("failed to read stdin: {err}");
                        break;
                    }
                }
            }
            log::info!("stdin closed");
        })
        .expect("failed to spawn stdin reader thread");
    rx
}

/// Writes one message as a single JSON line and flushes it.
pub fn send<T: Serialize>(message: &T) {
    let mut line = match serde_json::to_string(message) {
        Ok(line) => line,
        Err(err) => {
            log::error!("failed to serialize message: {err}");
            return;
        }
    };
    line.push('\n');
    let mut stdout = std::io::stdout().lock();
    if let Err(err) = stdout
        .write_all(line.as_bytes())
        .and_then(|_| stdout.flush())
    {
        log::error!("failed to write to stdout: {err}");
    }
}
