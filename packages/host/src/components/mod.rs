//! Adapters from tree nodes to gpui-component components (PROTOCOL.md section 5).
//!
//! Stateful components (Input, Checkbox) keep their state in the host and report
//! it with `change` events. With `controlled: true` the host does not change the
//! state itself: it only reports the requested value and waits for `setProps`.

pub mod accordion;
pub mod basic;
pub mod button;
pub mod calendar;
pub mod canvas;
pub mod carousel;
pub mod chart;
pub mod chat;
pub mod checkbox;
pub mod color_picker;
pub mod combobox;
pub mod command;
pub mod date_picker;
pub mod diff;
pub mod input;
pub mod keyboard;
pub mod layout;
pub mod menu;
pub mod otp;
pub mod overlays;
pub mod radio;
pub mod select;
pub mod settings;
pub mod sidebar;
pub mod slider;
pub mod switch;
pub mod table;
pub mod tabs;
pub mod time_field;
pub mod toggle;
pub mod tree;

use gpui_kit::{App, Entity};
use serde_json::Value;

use crate::Session;
use crate::protocol::{HostMessage, NodeId, Props};
use crate::schema::NodeType;
use crate::transport;
use crate::tree::{Node, Tree};

/// Asset path of a bundled icon (`folder` → `icons/folder.svg`).
pub fn icon_path(name: &str) -> String {
    format!("icons/{name}.svg")
}

/// Whether `name` is one of the icons bundled with gpui-component's default assets.
pub fn icon_exists(name: &str) -> bool {
    use gpui_kit::AssetSource as _;
    // shortcut: only the 101 default icons; registering `AllAssets` gives all 1,830 Lucide icons (+1 MB).
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && matches!(gpui_kit::assets::Assets.load(&icon_path(name)), Ok(Some(_)))
}

fn prop_str<'a>(node: &'a Node, key: &str) -> Option<&'a str> {
    node.props.get(key).and_then(Value::as_str)
}

fn prop_bool(node: &Node, key: &str) -> bool {
    node.props
        .get(key)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn is_controlled(tree: &Tree, id: NodeId) -> bool {
    tree.node(id)
        .is_some_and(|node| prop_bool(node, "controlled"))
}

/// Sends an event if the node subscribed to it (events are opt-in).
fn emit(tree: &Tree, id: NodeId, event: &str, value: Option<Value>, checked: Option<bool>) {
    if tree.wants(id, event) {
        transport::send(&HostMessage::Event {
            id,
            event: event.into(),
            value,
            checked,
            paths: None,
        });
    }
}

fn emit_change(tree: &Tree, id: NodeId, value: Option<Value>, checked: Option<bool>) {
    emit(tree, id, "change", value, checked);
}

/// Click handler shared by `div` and `Button`.
pub fn emit_click(session: &Entity<Session>, id: NodeId, cx: &App) {
    clicked(&session.read(cx).tree, id);
}

// User actions, shared by the GPUI handlers and `simulate` (`--no-window`), so the
// conformance fixtures exercise the same logic as the window.

pub fn clicked(tree: &Tree, id: NodeId) {
    emit(tree, id, "click", None, None);
}

/// The user closed an `Alert` with `closable`.
pub fn alert_closed(tree: &Tree, id: NodeId) {
    emit(tree, id, "close", None, None);
}

/// The user copied a `Clipboard`'s value. In `--no-window` the system clipboard is not touched
/// (as with `readClipboard` and the native dialogs).
pub fn copied(tree: &Tree, id: NodeId, value: String) {
    emit(tree, id, "copied", Some(value.into()), None);
}

/// The user answered an open Dialog (`ok` or `cancel`): it closes, like a synced control.
pub fn dialog_answered(tree: &mut Tree, id: NodeId, event: &str) {
    tree.set_host_prop(id, "open", Value::Bool(false));
    emit(tree, id, event, None, None);
}

/// Files dropped on a `div`.
pub fn dropped(tree: &Tree, id: NodeId, paths: Vec<String>) {
    if tree.wants(id, "drop") {
        transport::send(&HostMessage::Event {
            id,
            event: "drop".into(),
            value: None,
            checked: None,
            paths: Some(paths),
        });
    }
}

/// The second click of a double click (the first one already sent `click`).
pub fn double_clicked(tree: &Tree, id: NodeId) {
    emit(tree, id, "doubleClick", None, None);
}

/// A key pressed while the node (or something inside it) has the focus.
pub fn key_down(tree: &Tree, id: NodeId, keystroke: &gpui_kit::Keystroke) {
    emit(
        tree,
        id,
        "keyDown",
        Some(keystroke.unparse(crate::keys::OS).into()),
        None,
    );
}

/// A checkbox or switch click: synced mode keeps the new state in the host, controlled mode
/// only reports it. Returns whether the host state changed.
pub fn checkbox_clicked(tree: &mut Tree, id: NodeId, checked: bool) -> bool {
    let synced = !is_controlled(tree, id);
    if synced {
        tree.set_host_prop(id, "checked", Value::Bool(checked));
    }
    emit_change(tree, id, None, Some(checked));
    synced
}

/// A user choice in a RadioGroup, Select, Slider or Tabs, reported as `value`; synced mode keeps it
/// in the host. Returns whether the host state changed.
pub fn value_chosen(tree: &mut Tree, id: NodeId, value: Value) -> bool {
    chosen(tree, id, "value", value)
}

/// `value_chosen` for a choice kept in another prop (a Table's `selected`, a Pagination's `page`).
pub fn prop_chosen(tree: &mut Tree, id: NodeId, key: &str, value: Value) -> bool {
    chosen(tree, id, key, value)
}

/// `value_chosen` for a choice kept in another prop (a Table's `selected`).
fn chosen(tree: &mut Tree, id: NodeId, key: &str, value: Value) -> bool {
    let synced = !is_controlled(tree, id);
    if synced {
        tree.set_host_prop(id, key, value.clone());
    }
    emit_change(tree, id, Some(value), None);
    synced
}

/// An `f32` from GPUI as the JSON number it reads as (`0.3`, not `0.30000001192092896`).
pub fn number(value: f32) -> Value {
    value
        .to_string()
        .parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64)
        .map_or(Value::Null, Value::Number)
}

fn prop_f32(node: &Node, key: &str) -> Option<f32> {
    node.props
        .get(key)
        .and_then(Value::as_f64)
        .map(|n| n as f32)
}

/// The `options` of a RadioGroup or Select (`tabs` of Tabs): `(value, label, disabled)`.
pub fn options(node: &Node, key: &str) -> Vec<(String, String, bool)> {
    let text = |option: &Value, key| {
        option
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    node.props
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|option| {
            (
                text(option, "value"),
                text(option, "label"),
                option.get("disabled") == Some(&Value::Bool(true)),
            )
        })
        .collect()
}

/// The content of a DatePicker's, TimeField's or ColorPicker's `value` (and a ColorPicker's
/// `palette`), checked against the node as the op leaves it: `op` over `base`, where `null` removes.
pub fn check_form_value(
    node_type: NodeType,
    base: Option<&Props>,
    op: &Props,
) -> Result<(), String> {
    let get = |key: &str| match op.get(key) {
        Some(Value::Null) => None,
        Some(value) => Some(value),
        None => base.and_then(|props| props.get(key)),
    };
    let flag = |key| get(key).and_then(Value::as_bool).unwrap_or(false);
    let value = get("value").and_then(Value::as_str);
    let (ok, format) = match node_type {
        NodeType::DatePicker if flag("range") => (
            value.is_none_or(|v| date_picker::parse(v, true).is_some()),
            "'YYYY-MM-DD/YYYY-MM-DD' (start not after end)",
        ),
        NodeType::DatePicker => (
            value.is_none_or(|v| date_picker::parse(v, false).is_some()),
            "'YYYY-MM-DD'",
        ),
        NodeType::TimeField if flag("seconds") => (
            value.is_none_or(|v| time_field::parse(v, true).is_some()),
            "'HH:MM:SS'",
        ),
        NodeType::TimeField => (
            value.is_none_or(|v| time_field::parse(v, false).is_some()),
            "'HH:MM'",
        ),
        NodeType::ColorPicker => {
            let palette = get("palette")
                .and_then(Value::as_array)
                .into_iter()
                .flatten();
            if let Some(bad) = palette
                .into_iter()
                .find(|color| color.as_str().and_then(color_picker::parse).is_none())
            {
                return Err(format!(
                    "Prop 'palette' of ColorPicker must hold '#RRGGBB' or '#RRGGBBAA' colors, got {bad}"
                ));
            }
            (
                value.is_none_or(|v| color_picker::parse(v).is_some()),
                "'#RRGGBB' or '#RRGGBBAA'",
            )
        }
        _ => return Ok(()),
    };
    if ok {
        Ok(())
    } else {
        Err(format!(
            "Prop 'value' of {} must be {format}, got '{}'",
            node_type.name(),
            value.unwrap_or_default()
        ))
    }
}

/// Checks an `options` list: objects with `value` and `label` strings and an optional `disabled`.
pub fn check_options(value: &Value) -> Result<(), String> {
    let options = value.as_array().ok_or("a list of options")?;
    for (ix, option) in options.iter().enumerate() {
        let ok = option.get("value").is_some_and(Value::is_string)
            && option.get("label").is_some_and(Value::is_string)
            && option
                .get("disabled")
                .is_none_or(|d| d.is_boolean() || d.is_null());
        if !ok {
            return Err(format!(
                "option {ix} needs 'value' and 'label' strings (and an optional boolean 'disabled')"
            ));
        }
    }
    Ok(())
}

/// A user edit of an Input. Returns the value to revert to when the input is controlled
/// and the client has not accepted this text.
pub fn input_edited(tree: &Tree, id: NodeId, text: &str) -> Option<String> {
    emit_change(tree, id, Some(text.into()), None);
    if !is_controlled(tree, id) {
        return None;
    }
    let accepted = tree
        .node(id)
        .and_then(|node| prop_str(node, "value"))
        .unwrap_or("");
    (text != accepted).then(|| accepted.to_owned())
}

pub fn input_submitted(tree: &Tree, id: NodeId, text: String) {
    emit(tree, id, "submit", Some(text.into()), None);
}
