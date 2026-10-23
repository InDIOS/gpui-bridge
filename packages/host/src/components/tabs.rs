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

/// A `Tab` node's chip: its `label`, then its children (an `Icon`, a `text`, a `Badge`…) inside,
/// and a trailing control (`suffix`, see `is_control`) after them. The node's `a11y.label`
/// names the chip over its `label`: a tab of nodes has no text of its own.
pub fn chip(node: &Node, mut children: Vec<AnyElement>, suffix: Option<AnyElement>) -> Chip {
    let (value, disabled) = choice(node);
    let label = prop_str(node, "label").map(str::to_owned);
    let mut tab = Tab::new().disabled(disabled);
    if children.is_empty() {
        if let Some(label) = &label {
            tab = tab.label(label.clone());
        }
    } else {
        // gpui-component puts no gap between its label and the children of a chip, so a chip
        // of nodes draws its label itself, first in a row of its own that has one.
        if let Some(label) = &label {
            children.insert(0, SharedString::from(label.clone()).into_any_element());
        }
        tab = tab.child(h_flex().items_center().gap_2().children(children));
    }
    if let Some(name) = a11y::of(node).and_then(|spec| spec.label).or(label) {
        tab = tab.aria_label(name);
    }
    if let Some(suffix) = suffix {
        tab = tab.suffix(suffix);
    }
    (value, disabled, tab)
}

/// Whether the last child of a `Tab` goes in the chip's suffix: a control there (a close
/// button) keeps its size, where the box of the label and the children (fixed height,
/// clipped) would cut it.
pub fn is_control(node: &Node) -> bool {
    matches!(
        node.node_type,
        NodeType::Button | NodeType::Checkbox | NodeType::Switch
    )
}

/// The `a11y` of a `Tabs`, for the wrapper that names it: the bar already is a `tablist`
/// (gpui-component), so the wrapper is a `group` around it unless the client asks for another role.
pub fn named(mut spec: a11y::A11y) -> a11y::A11y {
    if matches!(spec.role, None | Some(Role::TabList)) {
        spec.role = Some(Role::Group);
    }
    spec
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
    let values: Vec<String> = chips.iter().map(|(value, ..)| value.clone()).collect();
    let session = session.clone();
    let bar = TabBar::new(("node", id))
        .with_variant(variant)
        .children(chips.into_iter().map(|(_, _, tab)| tab))
        .on_click(move |ix, _, cx| {
            // The selected tab again is not a change (gpui-component never reports a disabled one).
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
        });
    match selected {
        Some(ix) => bar.selected_index(ix),
        None => bar,
    }
}
