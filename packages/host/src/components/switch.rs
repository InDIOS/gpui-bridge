use gpui_kit::component::Disableable;
use gpui_kit::component::switch::Switch;
use gpui_kit::*;

use super::{checkbox_clicked, prop_bool, prop_str};
use crate::Session;
use crate::protocol::NodeId;
use crate::tree::Node;

/// Props: `checked`, `label`, `disabled`, `controlled`. Works like `Checkbox`.
pub fn build(id: NodeId, node: &Node, session: &Entity<Session>) -> Switch {
    let session = session.clone();
    let switch = Switch::new(("node", id))
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
        Some(label) => switch.label(label.to_owned()),
        None => switch,
    }
}
