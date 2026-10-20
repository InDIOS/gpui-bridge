//! System actions (PROTOCOL.md section 13): file dialogs, clipboard, opening URLs and files.
//! They run when they arrive, not on `commit`; the ones with an answer reply with `result`.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use gpui_kit::component::WindowExt;
use gpui_kit::component::notification::{Notification, NotificationType};
use gpui_kit::*;
use serde_json::{Map, Value};

use crate::protocol::{ClientMessage, ErrorCode, HostMessage, NotifyKind, ProtocolError};
use crate::transport;
use crate::windows::Windows;

/// Groups the client's toasts, so one with the same `id` replaces the previous one.
struct ClientNotification;

/// Rules serde cannot express. Checked in both modes, so `--no-window` reports them too.
pub fn check(action: &ClientMessage) -> Result<(), ProtocolError> {
    match action {
        ClientMessage::OpenDialog {
            files: Some(false),
            directories: None | Some(false),
            ..
        } => Err(ProtocolError {
            code: ErrorCode::InvalidJson,
            message: "'openDialog' must allow files or directories".into(),
            op: Some("openDialog".into()),
        }),
        _ => Ok(()),
    }
}

/// Runs an action; dialogs answer later, when the user closes them.
pub fn run(action: ClientMessage, windows: &RefCell<Windows>, cx: &mut App) {
    match action {
        ClientMessage::Notify {
            window,
            id,
            kind,
            title,
            message,
            autohide,
            system,
        } => {
            let window = window.unwrap_or(1);
            let Some(handle) = windows.borrow().handle(window) else {
                let message = format!("Window {window} is not open");
                transport::send(&HostMessage::from(ProtocolError {
                    code: ErrorCode::InvalidOperation,
                    message,
                    op: Some("notify".into()),
                }));
                return;
            };
            let kind = match kind.unwrap_or(NotifyKind::Info) {
                NotifyKind::Info => NotificationType::Info,
                NotifyKind::Success => NotificationType::Success,
                NotifyKind::Warning => NotificationType::Warning,
                NotifyKind::Error => NotificationType::Error,
            };
            let mut note = Notification::new()
                .message(message)
                .with_type(kind)
                .autohide(autohide.unwrap_or(true));
            if let Some(title) = title {
                note = note.title(title);
            }
            if let Some(id) = id {
                note = note
                    .id1::<ClientNotification>(SharedString::from(id.clone()))
                    .on_click(move |_, _, _| {
                        transport::send(&HostMessage::Notification {
                            id: id.clone(),
                            window,
                        });
                    });
            }
            if system == Some(true) {
                note = note.in_app_and_system();
            }
            if let Err(err) = handle.update(cx, |_, window, cx| window.push_notification(note, cx))
            {
                log::warn!("failed to show a notification: {err:#}");
            }
        }
        ClientMessage::OpenDialog {
            request,
            files,
            directories,
            multiple,
            prompt,
        } => {
            let answer = cx.prompt_for_paths(PathPromptOptions {
                files: files.unwrap_or(true),
                directories: directories.unwrap_or(false),
                multiple: multiple.unwrap_or(false),
                prompt: prompt.map(Into::into),
            });
            cx.spawn(async move |_| {
                let paths = picked(answer.await)
                    .map(|paths| paths.iter().map(|path| text(path)).collect::<Vec<_>>());
                reply(request, "paths", paths.map_or(Value::Null, Value::from));
            })
            .detach();
        }
        ClientMessage::SaveDialog {
            request,
            directory,
            name,
        } => {
            // Without a directory, the host's working directory (by default the client's).
            let directory = directory
                .map(PathBuf::from)
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_default();
            let answer = cx.prompt_for_new_path(&directory, name.as_deref());
            cx.spawn(async move |_| {
                let path = picked(answer.await).map(|path| text(&path));
                reply(request, "path", path.map_or(Value::Null, Value::from));
            })
            .detach();
        }
        ClientMessage::ReadClipboard { request } => {
            let text = cx.read_from_clipboard().and_then(|item| item.text());
            reply(request, "text", text.map_or(Value::Null, Value::from));
        }
        ClientMessage::WriteClipboard { text } => {
            cx.write_to_clipboard(ClipboardItem::new_string(text))
        }
        ClientMessage::OpenUrl { url } => cx.open_url(&url),
        ClientMessage::OpenPath { path } => cx.open_with_system(Path::new(&path)),
        other => unreachable!("{} is not a system action", other.op_name()),
    }
}

/// `--no-window`: nothing to show or open, so dialogs and the clipboard answer `null`.
pub fn run_without_window(action: ClientMessage) {
    match action {
        ClientMessage::OpenDialog { request, .. } => reply(request, "paths", Value::Null),
        ClientMessage::SaveDialog { request, .. } => reply(request, "path", Value::Null),
        ClientMessage::ReadClipboard { request } => reply(request, "text", Value::Null),
        _ => {}
    }
}

/// A cancelled dialog and one that failed both answer `null`; a failure is logged.
fn picked<T, E: std::fmt::Display, C>(answer: Result<Result<Option<T>, E>, C>) -> Option<T> {
    match answer {
        Ok(Ok(picked)) => picked,
        Ok(Err(err)) => {
            log::warn!("file dialog failed: {err:#}");
            None
        }
        Err(_) => None,
    }
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn reply(request: u64, key: &str, value: Value) {
    transport::send(&HostMessage::Result {
        request,
        body: Map::from_iter([(key.to_owned(), value)]),
    });
}
