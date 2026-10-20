use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::*;
use serde_json::Value;

use super::{options, prop_bool, prop_str, value_chosen};
use crate::Session;
use crate::protocol::NodeId;
use crate::tree::Node;

/// Props: `options`, `value`, `direction`, `disabled`, `controlled`. Choosing an option reports
/// its `value` with `change`; a `value` that is not an option selects nothing.
pub fn build(id: NodeId, node: &Node, session: &Entity<Session>) -> RadioGroup {
    let options = options(node, "options");
    let current = prop_str(node, "value").map(str::to_owned);
    let selected = current
        .as_deref()
        .and_then(|value| options.iter().position(|(option, ..)| option == value));
    let group = match prop_str(node, "direction") {
        Some("horizontal") => RadioGroup::horizontal(("node", id)),
        _ => RadioGroup::vertical(("node", id)),
    };
    let values: Vec<String> = options.iter().map(|(value, ..)| value.clone()).collect();
    let session = session.clone();
    group
        .selected_index(selected)
        .disabled(prop_bool(node, "disabled"))
        .children(
            options
                .into_iter()
                .enumerate()
                .map(|(ix, (_, label, disabled))| {
                    Radio::new(SharedString::from(format!("radio-{id}-{ix}")))
                        .label(label)
                        .disabled(disabled)
                }),
        )
        .on_click(move |ix, _, cx| {
            // Clicking the selected option again is not a change.
            let Some(value) = values
                .get(*ix)
                .filter(|value| Some(*value) != current.as_ref())
            else {
                return;
            };
            session.update(cx, |session, cx| {
                if value_chosen(&mut session.tree, id, Value::String(value.clone())) {
                    cx.notify();
                }
            });
        })
}
