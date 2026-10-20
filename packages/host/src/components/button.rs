use gpui_kit::component::Disableable;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::*;

use super::{emit_click, prop_bool, prop_str};
use crate::Session;
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
