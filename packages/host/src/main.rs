mod a11y;
mod animation;
mod components;
mod image_data;
mod keys;
mod protocol;
mod render;
mod schema;
mod simulate;
mod styles;
mod system;
mod transport;
mod tray;
mod tree;
mod windows;

use std::cell::RefCell;
use std::rc::Rc;

use futures::StreamExt;
use gpui_kit::*;
use protocol::{ClientMessage, ErrorCode, HostMessage, PROTOCOL_VERSION};
use tree::Tree;

fn main() {
    // `--version` is for packaging checks (CI asks the staged asset what it is); it does not
    // start a session, so stdout carries no protocol here.
    if std::env::args().any(|arg| arg == "--version") {
        println!(
            "gpui-bridge-host {} ({}-{}, protocol {})",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            PROTOCOL_VERSION
        );
        return;
    }

    // Logs go to stderr; stdout is reserved for the protocol.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Stderr)
        .init();

    let no_window = std::env::args().any(|arg| arg == "--no-window");
    let messages = transport::spawn_stdin_reader();

    if no_window {
        let mut session = Session::new(true);
        for line in futures::executor::block_on_stream(messages) {
            if let Flow::Exit = session.handle_line(&line) {
                break;
            }
        }
        return;
    }

    // Bundled icons: components such as Checkbox draw their marks from these SVGs.
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            // Windows shows system notifications (`notify` with `system`) only for an app with an identity.
            // shortcut: one fixed identity for every app; let the client name it if apps need their own.
            cx.set_app_identity("gpui-bridge", "gpui-bridge");
            // The host decides when to quit: after the user closes the last window (unless there is a tray),
            // after the client removes the tray with no window open, or when stdin ends.
            // Windows closed by the client with `closeWindow` must not end the app.
            cx.set_quit_mode(QuitMode::Explicit);

            let session = cx.new(|_| Session::new(false));
            let windows = Rc::new(RefCell::new(windows::Windows::default()));
            let tray = RefCell::new(tray::Tray::default());
            tray::init();
            cx.on_window_closed({
                let (session, windows) = (session.clone(), windows.clone());
                move |cx, closed| windows::window_closed(&session, &windows, closed, cx)
            })
            .detach();

            cx.spawn(async move |cx| {
                let mut messages = messages;
                while let Some(line) = messages.next().await {
                    let flow = cx.update(|cx| {
                        session.update(cx, |session, cx| {
                            let flow = session.handle_line(&line);
                            if let Flow::Render = flow {
                                cx.notify();
                            }
                            flow
                        })
                    });
                    match flow {
                        Flow::Continue => {}
                        Flow::Action(action) => cx.update(|cx| system::run(action, &windows, cx)),
                        Flow::Render => cx.update(|cx| {
                            render::reload_images(&session, cx);
                            windows::sync_windows(&session, &windows, cx);
                            tray::sync(&session, &tray, cx);
                            // Removing the tray with no window open leaves nothing to use: done.
                            if session.read(cx).tree.tray_removed_and_idle() {
                                cx.quit();
                            }
                        }),
                        Flow::Exit => break,
                    }
                }
                // The client went away or must be dropped: nothing can drive the UI anymore.
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
}

/// An error, carrying the line's `request` if it has one: a rejected system action never gets
/// its `result`, so the client must learn which request failed.
fn with_request(err: protocol::ProtocolError, line: &str) -> HostMessage {
    let request = serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .and_then(|value| value.get("request")?.as_u64());
    match HostMessage::from(err) {
        HostMessage::Error {
            code, message, op, ..
        } => HostMessage::Error {
            code,
            message,
            op,
            request,
        },
        other => other,
    }
}

enum Flow {
    Continue,
    /// A commit was applied: the UI must re-render.
    Render,
    /// A system action (PROTOCOL.md section 13), run by the caller with the GPUI app.
    Action(ClientMessage),
    Exit,
}

struct Session {
    handshake_done: bool,
    tree: Tree,
    /// `--no-window` mode: dump the tree to stderr after each commit and accept `simulate`.
    no_window: bool,
}

impl Session {
    fn new(no_window: bool) -> Self {
        Self {
            handshake_done: false,
            tree: Tree::default(),
            no_window,
        }
    }

    /// Handles one client line. Invalid input is answered with an `error` message and never stops the host.
    fn handle_line(&mut self, line: &str) -> Flow {
        let message = protocol::parse_client_message(line);

        // Until the protocol version is agreed, nothing else can be interpreted safely.
        if !self.handshake_done && !matches!(message, Ok(ClientMessage::Hello { .. })) {
            log::warn!("ignoring message received before hello: {line}");
            return Flow::Continue;
        }

        if self.no_window {
            match simulate::handle(line, &mut self.tree) {
                Some(Ok(simulate::Outcome::Done)) => return Flow::Continue,
                Some(Ok(simulate::Outcome::Exit)) => return Flow::Exit,
                Some(Err(err)) => {
                    transport::send(&HostMessage::from(err));
                    return Flow::Continue;
                }
                None => {}
            }
        }

        let message = match message {
            Ok(message) => message,
            Err(err) => {
                log::warn!("rejected message: {}", err.message);
                transport::send(&with_request(err, line));
                return Flow::Continue;
            }
        };

        match message {
            ClientMessage::Hello { protocol } if protocol == PROTOCOL_VERSION => {
                self.handshake_done = true;
                transport::send(&HostMessage::Ready {
                    protocol: PROTOCOL_VERSION,
                });
            }
            ClientMessage::Hello { protocol } => {
                transport::send(&HostMessage::Error {
                    code: ErrorCode::ProtocolMismatch,
                    message: format!(
                        "Host speaks protocol {PROTOCOL_VERSION}, client requested {protocol}"
                    ),
                    op: Some("hello".into()),
                    request: None,
                });
                return Flow::Exit;
            }
            ClientMessage::Commit => {
                for err in self.tree.commit() {
                    log::warn!("operation failed: {}", err.message);
                    transport::send(&HostMessage::from(err));
                }
                if self.no_window {
                    log::info!("tree after commit:\n{}", self.tree.dump());
                    if self.tree.tray_removed_and_idle() {
                        return Flow::Exit;
                    }
                }
                return Flow::Render;
            }
            action if action.is_action() => {
                if let Err(err) = system::check(&action) {
                    transport::send(&with_request(err, line));
                } else if self.no_window {
                    system::run_without_window(action);
                } else {
                    return Flow::Action(action);
                }
            }
            op => self.tree.queue(op),
        }
        Flow::Continue
    }
}
