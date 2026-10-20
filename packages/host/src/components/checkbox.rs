use super::{checkbox_clicked, prop_bool, prop_str};
use crate::Session;
use crate::protocol::NodeId;
use crate::tree::Node;
use gpui_kit::component::Disableable;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::*;

/// Props: `checked`, `label`, `disabled`, `controlled`. A click toggles the
/// host-side state (unless controlled) and reports it with `change`.
pub fn build(id: NodeId, node: &Node, session: &Entity<Session>) -> Checkbox {
    let session = session.clone();
    let checkbox = Checkbox::new(("node", id))
        .checked(prop_bool(node, "checked"))
        .disabled(prop_bool(node, "disabled"))
        .on_click(move |checked, _, cx| {
            session.update(cx, |session, cx| {
                if checkbox_clicked(&mut session.tree, id, *checked) {
                    cx.notify();
                }
            });
        });
    match prop_str(node, "label") {
        Some(label) => checkbox.label(label.to_owned()),
        None => checkbox,
    }
}
