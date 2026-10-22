use gpui_kit::component::h_flex;
use gpui_kit::component::tab::{Tab, TabBar, TabVariant};
use gpui_kit::*;
use serde_json::Value;

use super::{options, prop_bool, prop_str, value_chosen};
use crate::Session;
use crate::a11y;
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree};

/// A tab of a `Tabs`: the `value` it reports, whether it is disabled, and the chip to draw.
pub type Chip = (String, bool, Tab);

/// A `Tab` node's chip: its `label`, then its children (an `Icon`, a `text`, a `Badge`…) inside.
/// With no `label`, the node's `a11y.label` names the chip: a tab of nodes has no text of its own.
pub fn chip(node: &Node, children: Vec<AnyElement>) -> Chip {
    let (value, disabled) = choice(node);
    let mut tab = Tab::new().disabled(disabled);
    match prop_str(node, "label") {
        Some(label) => tab = tab.label(label.to_owned()),
        None => {
            if let Some(label) = a11y::of(node).and_then(|spec| spec.label) {
                tab = tab.aria_label(label);
            }
        }
    }
    if !children.is_empty() {
        // gpui-component only spaces the parts of a chip it builds itself (its label, a prefix
        // and a suffix), so the nodes of a chip of our own are laid out in a row that has a gap.
        tab = tab.child(h_flex().items_center().gap_2().children(children));
    }
    (value, disabled, tab)
}

/// The `(value, disabled)` of a `Tabs`: its `Tab` children, or its `tabs` entries.
pub fn choices(tree: &Tree, node: &Node) -> Vec<(String, bool)> {
    let children: Vec<(String, bool)> = node
        .children
        .iter()
        .filter_map(|&child| {
            tree.node(child)
                .filter(|tab| tab.node_type == NodeType::Tab)
                .map(choice)
        })
        .collect();
    match children.is_empty() {
        false => children,
        true => options(node, "tabs")
            .into_iter()
            .map(|(value, _, disabled)| (value, disabled))
            .collect(),
    }
}

/// What a `Tab` node reports: its `value` and whether it is disabled.
fn choice(node: &Node) -> (String, bool) {
    (
        prop_str(node, "value").unwrap_or_default().to_owned(),
        prop_bool(node, "disabled"),
    )
}

/// Props: `tabs`, `value`, `variant`, `controlled`. Only the bar: the app shows the panel of
/// the chosen tab. Choosing a tab reports its `value` with `change`. The `chips` of the caller
/// (the `Tab` children of the node) replace `tabs` when it gives any.
pub fn build(id: NodeId, node: &Node, chips: Vec<Chip>, session: &Entity<Session>) -> TabBar {
    let chips = match chips.is_empty() {
        false => chips,
        true => options(node, "tabs")
            .into_iter()
            .map(|(value, label, disabled)| {
                (value, disabled, Tab::new().label(label).disabled(disabled))
            })
            .collect(),
    };
    let current = prop_str(node, "value").map(str::to_owned);
    let selected = current
        .as_deref()
        .and_then(|value| chips.iter().position(|(tab, ..)| tab == value));
    let variant = match prop_str(node, "variant") {
        Some("outline") => TabVariant::Outline,
        Some("pill") => TabVariant::Pill,
        Some("segmented") => TabVariant::Segmented,
        Some("underline") => TabVariant::Underline,
        _ => TabVariant::Tab,
    };
    let tab_choices: Vec<(String, bool)> = chips
        .iter()
        .map(|(value, disabled, _)| (value.clone(), *disabled))
        .collect();
    let session = session.clone();
    let bar = TabBar::new(("node", id))
        .with_variant(variant)
        .children(chips.into_iter().map(|(_, _, tab)| tab))
        .on_click(move |ix, _, cx| {
            // A disabled tab, or the selected one again, is not a change.
            let Some((value, _)) = tab_choices
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
