use gpui_kit::component::tab::{Tab, TabBar, TabVariant};
use gpui_kit::*;
use serde_json::Value;

use super::{options, prop_str, value_chosen};
use crate::Session;
use crate::protocol::NodeId;
use crate::tree::Node;

/// Props: `tabs`, `value`, `variant`, `controlled`. Only the bar: the app shows the panel of
/// the chosen tab. Choosing a tab reports its `value` with `change`.
pub fn build(id: NodeId, node: &Node, session: &Entity<Session>) -> TabBar {
    let tabs = options(node, "tabs");
    let current = prop_str(node, "value").map(str::to_owned);
    let selected = current
        .as_deref()
        .and_then(|value| tabs.iter().position(|(tab, ..)| tab == value));
    let variant = match prop_str(node, "variant") {
        Some("outline") => TabVariant::Outline,
        Some("pill") => TabVariant::Pill,
        Some("segmented") => TabVariant::Segmented,
        Some("underline") => TabVariant::Underline,
        _ => TabVariant::Tab,
    };
    let choices: Vec<(String, bool)> = tabs
        .iter()
        .map(|(value, _, disabled)| (value.clone(), *disabled))
        .collect();
    let session = session.clone();
    let bar = TabBar::new(("node", id))
        .with_variant(variant)
        .children(
            tabs.into_iter()
                .map(|(_, label, disabled)| Tab::new().label(label).disabled(disabled)),
        )
        .on_click(move |ix, _, cx| {
            // A disabled tab, or the selected one again, is not a change.
            let Some((value, _)) = choices
                .get(*ix)
                .filter(|(value, disabled)| !disabled && Some(value) != current.as_ref())
            else {
                return;
            };
            session.update(cx, |session, cx| {
                if value_chosen(&mut session.tree, id, Value::String(value.clone())) {
                    cx.notify();
                }
            });
        });
    match selected {
        Some(ix) => bar.selected_index(ix),
        None => bar,
    }
}
