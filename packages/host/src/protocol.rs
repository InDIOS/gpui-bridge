//! Protocol message types. Source of truth together with `client-node/src/protocol.ts`
//! and PROTOCOL.md.

use std::sync::Arc;

use gpui_kit::component::theme::ThemeSet;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

pub const PROTOCOL_VERSION: u32 = 1;

pub type NodeId = u64;

/// Client-assigned window id; `window` without `id` means window 1.
pub type WindowId = u64;

/// Node props. A `null` value in `setProps` removes the prop, so nulls are kept.
pub type Props = Map<String, Value>;

/// Messages from the client (stdin).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum ClientMessage {
    Hello {
        protocol: u32,
    },
    Window {
        id: Option<WindowId>,
        title: Option<String>,
        width: Option<f32>,
        height: Option<f32>,
        /// Initial position, in logical pixels; like `frameless`, read when the window opens.
        x: Option<f32>,
        y: Option<f32>,
        #[serde(rename = "minWidth")]
        min_width: Option<f32>,
        #[serde(rename = "minHeight")]
        min_height: Option<f32>,
        root: Option<NodeId>,
        theme: Option<WindowTheme>,
        #[serde(rename = "themeSet")]
        theme_set: Option<CustomThemes>,
        frameless: Option<bool>,
        /// Minimizes (`true`) or restores (`false`) on commit. An action, not a remembered state.
        minimized: Option<bool>,
        /// Window events the client wants (`focus`, `blur`, `resize`, `move`, `visibility`);
        /// replaces the previous list.
        events: Option<Vec<String>>,
    },
    Create {
        id: NodeId,
        #[serde(rename = "type")]
        node_type: String,
        #[serde(default)]
        props: Props,
        #[serde(default)]
        events: Vec<String>,
    },
    Append {
        parent: NodeId,
        child: NodeId,
    },
    InsertBefore {
        parent: NodeId,
        child: NodeId,
        before: NodeId,
    },
    SetProps {
        id: NodeId,
        props: Props,
    },
    SetEvents {
        id: NodeId,
        events: Vec<String>,
    },
    Remove {
        id: NodeId,
    },
    CloseWindow {
        id: WindowId,
    },
    /// The system tray icon (PROTOCOL.md section 11); `menu` is checked like `contextMenu`.
    Tray {
        icon: Option<String>,
        tooltip: Option<String>,
        menu: Option<Value>,
    },
    RemoveTray,
    /// App-wide keyboard shortcuts (PROTOCOL.md section 12); each message replaces the list.
    Shortcuts {
        keys: Vec<String>,
    },
    /// Gives a node the keyboard focus.
    Focus {
        id: NodeId,
    },
    // System actions (PROTOCOL.md section 13): run when they arrive, not on `commit`.
    OpenDialog {
        request: u64,
        files: Option<bool>,
        directories: Option<bool>,
        multiple: Option<bool>,
        prompt: Option<String>,
    },
    SaveDialog {
        request: u64,
        directory: Option<String>,
        name: Option<String>,
    },
    ReadClipboard {
        request: u64,
    },
    WriteClipboard {
        text: String,
    },
    OpenUrl {
        url: String,
    },
    OpenPath {
        path: String,
    },
    /// A toast in a window (PROTOCOL.md section 14); also an action.
    Notify {
        window: Option<WindowId>,
        id: Option<String>,
        #[serde(rename = "type")]
        kind: Option<NotifyKind>,
        title: Option<String>,
        message: String,
        autohide: Option<bool>,
        system: Option<bool>,
    },
    Commit,
}

/// The look of a `notify` toast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotifyKind {
    Info,
    Success,
    Warning,
    Error,
}

/// Color mode of the gpui-component theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowTheme {
    Light,
    Dark,
}

/// A gpui-component theme file (`ThemeSet`). Compared by identity, so every `window`
/// message that carries one applies it again.
#[derive(Debug, Clone)]
pub struct CustomThemes(pub Arc<ThemeSet>);

impl PartialEq for CustomThemes {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl<'de> Deserialize<'de> for CustomThemes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        ThemeSet::deserialize(deserializer).map(|set| Self(Arc::new(set)))
    }
}

/// Messages from the host (stdout).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HostMessage {
    Ready {
        protocol: u32,
    },
    Event {
        id: NodeId,
        event: String,
        /// Text for most events; a number for `Slider`.
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<Value>,
        /// Files dropped on a `div` (`drop`).
        #[serde(skip_serializing_if = "Option::is_none")]
        paths: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        checked: Option<bool>,
    },
    WindowClosed {
        id: WindowId,
    },
    /// A window event the client subscribed to in `window` (PROTOCOL.md section 3).
    /// `move` and `resize` carry the window's bounds; `visibility` carries `visible`.
    WindowEvent {
        id: WindowId,
        event: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        x: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        y: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        width: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        height: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        visible: Option<bool>,
    },
    Tray {
        event: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<String>,
    },
    Shortcut {
        keys: String,
        window: WindowId,
    },
    /// The user clicked a toast that has an `id`.
    Notification {
        id: String,
        window: WindowId,
    },
    /// The answer to a system action with a `request` id: `paths`, `path` or `text`.
    Result {
        request: u64,
        #[serde(flatten)]
        body: Map<String, Value>,
    },
    Error {
        code: ErrorCode,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        op: Option<String>,
        /// The `request` of a rejected system action, so the client stops waiting for its result.
        #[serde(skip_serializing_if = "Option::is_none")]
        request: Option<u64>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidJson,
    UnknownOp,
    UnknownNode,
    UnknownType,
    InvalidProps,
    InvalidOperation,
    ProtocolMismatch,
}

impl ClientMessage {
    /// System actions run when they arrive instead of on `commit`.
    pub fn is_action(&self) -> bool {
        matches!(
            self,
            Self::OpenDialog { .. }
                | Self::SaveDialog { .. }
                | Self::ReadClipboard { .. }
                | Self::WriteClipboard { .. }
                | Self::OpenUrl { .. }
                | Self::OpenPath { .. }
                | Self::Notify { .. }
        )
    }

    /// The wire name of the operation, used in `error` replies.
    pub fn op_name(&self) -> &'static str {
        match self {
            Self::Hello { .. } => "hello",
            Self::Window { .. } => "window",
            Self::Create { .. } => "create",
            Self::Append { .. } => "append",
            Self::InsertBefore { .. } => "insertBefore",
            Self::SetProps { .. } => "setProps",
            Self::SetEvents { .. } => "setEvents",
            Self::Remove { .. } => "remove",
            Self::CloseWindow { .. } => "closeWindow",
            Self::Tray { .. } => "tray",
            Self::RemoveTray => "removeTray",
            Self::Shortcuts { .. } => "shortcuts",
            Self::Focus { .. } => "focus",
            Self::OpenDialog { .. } => "openDialog",
            Self::SaveDialog { .. } => "saveDialog",
            Self::ReadClipboard { .. } => "readClipboard",
            Self::WriteClipboard { .. } => "writeClipboard",
            Self::OpenUrl { .. } => "openUrl",
            Self::OpenPath { .. } => "openPath",
            Self::Notify { .. } => "notify",
            Self::Commit => "commit",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolError {
    pub code: ErrorCode,
    pub message: String,
    pub op: Option<String>,
}

impl From<ProtocolError> for HostMessage {
    fn from(err: ProtocolError) -> Self {
        HostMessage::Error {
            code: err.code,
            message: err.message,
            op: err.op,
            request: None,
        }
    }
}

/// Parses one line from the client, classifying failures into protocol error codes.
pub fn parse_client_message(line: &str) -> Result<ClientMessage, ProtocolError> {
    let error = |code, message: String, op: Option<&str>| ProtocolError {
        code,
        message,
        op: op.map(str::to_owned),
    };

    let value: Value = serde_json::from_str(line)
        .map_err(|err| error(ErrorCode::InvalidJson, format!("Invalid JSON: {err}"), None))?;
    let Some(object) = value.as_object() else {
        return Err(error(
            ErrorCode::InvalidJson,
            "Message must be a JSON object".into(),
            None,
        ));
    };
    let Some(op) = object.get("op").and_then(Value::as_str).map(str::to_owned) else {
        return Err(error(
            ErrorCode::UnknownOp,
            "Message has no string 'op' field".into(),
            None,
        ));
    };

    if !OPS.contains(&op.as_str()) {
        return Err(error(
            ErrorCode::UnknownOp,
            format!("Unknown op '{op}'"),
            Some(&op),
        ));
    }
    serde_json::from_value(value).map_err(|err| {
        error(
            ErrorCode::InvalidJson,
            format!("Invalid '{op}' message: {err}"),
            Some(&op),
        )
    })
}

/// Wire names of every `ClientMessage` variant (kept in sync by a test).
const OPS: [&str; 21] = [
    "hello",
    "window",
    "create",
    "append",
    "insertBefore",
    "setProps",
    "setEvents",
    "remove",
    "closeWindow",
    "tray",
    "removeTray",
    "shortcuts",
    "focus",
    "openDialog",
    "saveDialog",
    "readClipboard",
    "writeClipboard",
    "openUrl",
    "openPath",
    "notify",
    "commit",
];

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(line: &str) -> ClientMessage {
        parse_client_message(line).unwrap()
    }

    fn parse_err(line: &str) -> ProtocolError {
        parse_client_message(line).unwrap_err()
    }

    fn props(value: Value) -> Props {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn parses_hello() {
        assert_eq!(
            parse(r#"{"op":"hello","protocol":1}"#),
            ClientMessage::Hello { protocol: 1 }
        );
    }

    #[test]
    fn parses_window() {
        assert_eq!(
            parse(
                r#"{"op":"window","title":"My app","width":800,"height":600,"root":1,"theme":"dark"}"#
            ),
            ClientMessage::Window {
                id: None,
                title: Some("My app".into()),
                width: Some(800.0),
                height: Some(600.0),
                x: None,
                y: None,
                min_width: None,
                min_height: None,
                root: Some(1),
                theme: Some(WindowTheme::Dark),
                theme_set: None,
                frameless: None,
                minimized: None,
                events: None,
            }
        );
        assert_eq!(
            parse(r#"{"op":"window","title":"Renamed"}"#),
            ClientMessage::Window {
                id: None,
                title: Some("Renamed".into()),
                width: None,
                height: None,
                x: None,
                y: None,
                min_width: None,
                min_height: None,
                root: None,
                theme: None,
                theme_set: None,
                frameless: None,
                minimized: None,
                events: None
            }
        );
        // Position, minimum size, minimize/restore and window events (PROTOCOL.md section 3).
        assert_eq!(
            parse(
                r#"{"op":"window","id":2,"x":120,"y":80,"minWidth":320,"minHeight":200,"minimized":true,"events":["focus","resize"]}"#
            ),
            ClientMessage::Window {
                id: Some(2),
                title: None,
                width: None,
                height: None,
                x: Some(120.0),
                y: Some(80.0),
                min_width: Some(320.0),
                min_height: Some(200.0),
                root: None,
                theme: None,
                theme_set: None,
                frameless: None,
                minimized: Some(true),
                events: Some(vec!["focus".into(), "resize".into()])
            }
        );
        let err = parse_err(r#"{"op":"window","theme":"sepia"}"#);
        assert_eq!(
            (err.code, err.op.as_deref()),
            (ErrorCode::InvalidJson, Some("window"))
        );

        let msg = parse(
            r##"{"op":"window","themeSet":{"name":"Brand","themes":[{"name":"Brand Dark","mode":"dark","radius":12,"colors":{"primary.background":"#ff6600"}}]}}"##,
        );
        let ClientMessage::Window {
            theme_set: Some(CustomThemes(set)),
            ..
        } = msg
        else {
            panic!("{msg:?}")
        };
        assert_eq!((set.themes.len(), set.themes[0].radius), (1, Some(12)));
        assert_eq!(set.themes[0].colors.primary.as_deref(), Some("#ff6600"));
        let err = parse_err(r#"{"op":"window","themeSet":{"themes":[{"mode":"sepia"}]}}"#);
        assert_eq!(
            (err.code, err.op.as_deref()),
            (ErrorCode::InvalidJson, Some("window"))
        );
    }

    #[test]
    fn parses_create() {
        assert_eq!(
            parse(
                r#"{"op":"create","id":3,"type":"Button","props":{"label":"Save","variant":"primary"},"events":["click"]}"#
            ),
            ClientMessage::Create {
                id: 3,
                node_type: "Button".into(),
                props: props(json!({"label": "Save", "variant": "primary"})),
                events: vec!["click".into()],
            }
        );
        // props and events are optional.
        assert_eq!(
            parse(r#"{"op":"create","id":1,"type":"div"}"#),
            ClientMessage::Create {
                id: 1,
                node_type: "div".into(),
                props: Props::new(),
                events: vec![]
            }
        );
    }

    #[test]
    fn parses_tree_ops() {
        assert_eq!(
            parse(r#"{"op":"append","parent":1,"child":2}"#),
            ClientMessage::Append {
                parent: 1,
                child: 2
            }
        );
        assert_eq!(
            parse(r#"{"op":"insertBefore","parent":1,"child":3,"before":2}"#),
            ClientMessage::InsertBefore {
                parent: 1,
                child: 3,
                before: 2
            }
        );
        assert_eq!(
            parse(r#"{"op":"setEvents","id":3,"events":["click"]}"#),
            ClientMessage::SetEvents {
                id: 3,
                events: vec!["click".into()]
            }
        );
        assert_eq!(
            parse(r#"{"op":"remove","id":3}"#),
            ClientMessage::Remove { id: 3 }
        );
        assert_eq!(
            parse(r#"{"op":"closeWindow","id":2}"#),
            ClientMessage::CloseWindow { id: 2 }
        );
        assert_eq!(parse(r#"{"op":"commit"}"#), ClientMessage::Commit);
    }

    #[test]
    fn set_props_keeps_nulls() {
        let ClientMessage::SetProps { id, props } =
            parse(r#"{"op":"setProps","id":3,"props":{"disabled":true,"label":null}}"#)
        else {
            panic!("expected setProps");
        };
        assert_eq!(id, 3);
        assert_eq!(props.get("disabled"), Some(&json!(true)));
        assert_eq!(props.get("label"), Some(&Value::Null));
    }

    #[test]
    fn serializes_host_messages() {
        let cases = [
            (
                HostMessage::Ready { protocol: 1 },
                json!({"type": "ready", "protocol": 1}),
            ),
            (
                HostMessage::Event {
                    id: 3,
                    event: "click".into(),
                    value: None,
                    checked: None,
                    paths: None,
                },
                json!({"type": "event", "id": 3, "event": "click"}),
            ),
            (
                HostMessage::Event {
                    id: 5,
                    event: "change".into(),
                    value: Some("text".into()),
                    checked: None,
                    paths: None,
                },
                json!({"type": "event", "id": 5, "event": "change", "value": "text"}),
            ),
            (
                HostMessage::Event {
                    id: 7,
                    event: "change".into(),
                    value: None,
                    checked: Some(true),
                    paths: None,
                },
                json!({"type": "event", "id": 7, "event": "change", "checked": true}),
            ),
            (
                HostMessage::WindowClosed { id: 2 },
                json!({"type": "windowClosed", "id": 2}),
            ),
            (
                HostMessage::WindowEvent {
                    id: 2,
                    event: "resize".into(),
                    x: Some(100.0),
                    y: Some(80.0),
                    width: Some(400.0),
                    height: Some(300.0),
                    visible: None,
                },
                json!({"type": "windowEvent", "id": 2, "event": "resize", "x": 100.0, "y": 80.0, "width": 400.0, "height": 300.0}),
            ),
            (
                HostMessage::WindowEvent {
                    id: 1,
                    event: "visibility".into(),
                    x: None,
                    y: None,
                    width: None,
                    height: None,
                    visible: Some(false),
                },
                json!({"type": "windowEvent", "id": 1, "event": "visibility", "visible": false}),
            ),
            (
                HostMessage::Error {
                    code: ErrorCode::UnknownNode,
                    message: "Node 42 does not exist".into(),
                    op: Some("setProps".into()),
                    request: None,
                },
                json!({"type": "error", "code": "unknown_node", "message": "Node 42 does not exist", "op": "setProps"}),
            ),
            (
                HostMessage::Error {
                    code: ErrorCode::InvalidJson,
                    message: "bad".into(),
                    op: None,
                    request: None,
                },
                json!({"type": "error", "code": "invalid_json", "message": "bad"}),
            ),
        ];
        for (message, expected) in cases {
            assert_eq!(serde_json::to_value(&message).unwrap(), expected);
        }
    }

    #[test]
    fn serializes_every_error_code() {
        let codes = [
            (ErrorCode::InvalidJson, "invalid_json"),
            (ErrorCode::UnknownOp, "unknown_op"),
            (ErrorCode::UnknownNode, "unknown_node"),
            (ErrorCode::UnknownType, "unknown_type"),
            (ErrorCode::InvalidProps, "invalid_props"),
            (ErrorCode::InvalidOperation, "invalid_operation"),
            (ErrorCode::ProtocolMismatch, "protocol_mismatch"),
        ];
        for (code, expected) in codes {
            assert_eq!(serde_json::to_value(code).unwrap(), json!(expected));
        }
    }

    #[test]
    fn classifies_parse_errors() {
        let err = parse_err("{not json");
        assert_eq!((err.code, err.op), (ErrorCode::InvalidJson, None));

        assert_eq!(parse_err("[1,2]").code, ErrorCode::InvalidJson);
        assert_eq!(parse_err(r#"{"id":1}"#).code, ErrorCode::UnknownOp);

        let err = parse_err(r#"{"op":"explode"}"#);
        assert_eq!(
            (err.code, err.op.as_deref()),
            (ErrorCode::UnknownOp, Some("explode"))
        );

        let err = parse_err(r#"{"op":"create","type":"div"}"#);
        assert_eq!(
            (err.code, err.op.as_deref()),
            (ErrorCode::InvalidJson, Some("create"))
        );

        assert_eq!(
            parse_err(r#"{"op":"remove","id":"three"}"#).code,
            ErrorCode::InvalidJson
        );
    }

    #[test]
    fn op_name_matches_wire_name() {
        let lines = [
            r#"{"op":"hello","protocol":1}"#,
            r#"{"op":"window"}"#,
            r#"{"op":"create","id":1,"type":"div"}"#,
            r#"{"op":"append","parent":1,"child":2}"#,
            r#"{"op":"insertBefore","parent":1,"child":2,"before":3}"#,
            r#"{"op":"setProps","id":1,"props":{}}"#,
            r#"{"op":"setEvents","id":1,"events":[]}"#,
            r#"{"op":"remove","id":1}"#,
            r#"{"op":"closeWindow","id":1}"#,
            r#"{"op":"tray"}"#,
            r#"{"op":"removeTray"}"#,
            r#"{"op":"shortcuts","keys":[]}"#,
            r#"{"op":"focus","id":1}"#,
            r#"{"op":"openDialog","request":1}"#,
            r#"{"op":"saveDialog","request":1}"#,
            r#"{"op":"readClipboard","request":1}"#,
            r#"{"op":"writeClipboard","text":""}"#,
            r#"{"op":"openUrl","url":""}"#,
            r#"{"op":"openPath","path":""}"#,
            r#"{"op":"notify","message":""}"#,
            r#"{"op":"commit"}"#,
        ];
        for line in lines {
            let wire: Value = serde_json::from_str(line).unwrap();
            assert_eq!(parse(line).op_name(), wire["op"]);
        }
        assert_eq!(OPS.len(), lines.len());
    }

    #[test]
    fn ignores_unknown_fields() {
        // Newer clients may send extra fields; they must not break older hosts.
        assert_eq!(
            parse(r#"{"op":"remove","id":3,"extra":true}"#),
            ClientMessage::Remove { id: 3 }
        );
    }
}
