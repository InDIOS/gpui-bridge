use super::{checkbox_clicked, prop_bool, prop_str};
use crate::Session;
use crate::protocol::NodeId;
use crate::tree::Node;
use gpui_kit::component::Disableable;
use gpui_kit::component::button::{Toggle, ToggleVariant, ToggleVariants};
use gpui_kit::*;

/// Props: `checked`, `label`, `icon`, `variant` (ghost, outline), `tooltip`, `disabled`,
/// `controlled`. Works like `Checkbox`: a click toggles the host-side state (unless controlled)
/// and reports it with `change`. Its children are drawn after `label`/`icon`.
pub fn build(id: NodeId, node: &Node, session: &Entity<Session>) -> Toggle {
    let session = session.clone();
    let mut toggle = Toggle::new(("node", id))
        .checked(prop_bool(node, "checked"))
        .disabled(prop_bool(node, "disabled"))
        .on_click(move |checked, _, cx| {
            session.update(cx, |session, cx| {
                if checkbox_clicked(&mut session.tree, id, *checked) {
                    cx.notify();
                }
            });
        });
    if let Some(label) = prop_str(node, "label") {
        toggle = toggle.label(label.to_owned());
    }
    if let Some(icon) = super::basic::icon(node, "icon") {
        toggle = toggle.icon(icon);
    }
    if let Some(tooltip) = prop_str(node, "tooltip") {
        toggle = toggle.tooltip(tooltip.to_owned());
    }
    match prop_str(node, "variant") {
        Some("outline") => toggle.with_variant(ToggleVariant::Outline),
        Some("ghost") => toggle.with_variant(ToggleVariant::Ghost),
        _ => toggle,
    }
}
