use gpui_kit::component::Disableable;
use gpui_kit::component::button::{Button, ButtonVariants, DropdownButton};
use gpui_kit::component::clipboard::Clipboard;
use gpui_kit::*;
use serde_json::Value;

use super::{basic, copied, emit_click, menu, prop_bool, prop_str};
use crate::Session;
use crate::a11y;
use crate::protocol::NodeId;
use crate::tree::Node;

/// Props: `label`, `variant` (primary, secondary, ghost, danger), `disabled`. Events: `click`.
pub fn build(id: NodeId, node: &Node, session: &Entity<Session>) -> Button {
    let mut button = Button::new(("node", id))
        .label(prop_str(node, "label").unwrap_or("").to_owned())
        .disabled(prop_bool(node, "disabled"));
    if let Some(tooltip) = prop_str(node, "tooltip") {
        button = button.tooltip(tooltip.to_owned());
    }
    if node.events.contains("click") {
        let session = session.clone();
        button = button.on_click(move |_, _, cx| emit_click(&session, id, cx));
    }
    match prop_str(node, "variant") {
        Some("primary") => button.primary(),
        Some("secondary") => button.secondary(),
        Some("ghost") => button.ghost(),
        Some("danger") => button.danger(),
        // No variant: gpui-component's default look.
        _ => button,
    }
}

/// Props: as `Button`, plus `icon`, `items` (a menu list) and `menuLabel` (the name of the caret).
/// Events: `click` (the left half), `menu` (the chosen item's `id`).
pub fn dropdown(id: NodeId, node: &Node, session: &Entity<Session>) -> DropdownButton {
    // The left half is a `Button`: it keeps the label, the variant, the tooltip and `click`.
    let mut button = build(id, node, session);
    if let Some(icon) = basic::icon(node, "icon") {
        button = button.icon(icon);
    }
    let items = node
        .props
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut dropdown = DropdownButton::new(("node", id))
        .button(button)
        .disabled(prop_bool(node, "disabled"))
        .dropdown_menu(menu::builder(items, session, id));
    if prop_bool(node, "outline") {
        dropdown = dropdown.outline();
    }
    if let Some(label) = prop_str(node, "menuLabel") {
        dropdown = dropdown.menu_accessibility_label(label.to_owned());
    }
    dropdown
}

/// Props: `value` (the text copied on click), `tooltip`. Events: `copied` with that text.
/// The component reports a button named by `accessibility_label`, so it takes the node's
/// `a11y.label` (or its `tooltip`) and the host does not wrap it (see `node_raw`).
pub fn clipboard(id: NodeId, node: &Node, session: &Entity<Session>) -> Clipboard {
    let label = a11y::of(node)
        .and_then(|spec| spec.label)
        .or_else(|| prop_str(node, "tooltip").map(str::to_owned));
    let mut clipboard =
        Clipboard::new(("node", id)).value(prop_str(node, "value").unwrap_or("").to_owned());
    if let Some(label) = label {
        clipboard = clipboard.accessibility_label(label);
    }
    if let Some(tooltip) = prop_str(node, "tooltip") {
        clipboard = clipboard.tooltip(tooltip.to_owned());
    }
    if node.events.contains("copied") {
        let session = session.clone();
        clipboard = clipboard.on_copied(move |value, _, cx| {
            copied(&session.read(cx).tree, id, value.to_string());
        });
    }
    clipboard
}
