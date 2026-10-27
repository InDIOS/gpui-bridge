//! A Tab stop and arrow keys for the components gpui-component 0.8 leaves without them:
//! `Slider`, `Rating`, `Tabs` and `Pagination` (PROTOCOL.md section 16). A key makes the choice
//! a click would, so the synced and controlled modes work as they do with the pointer.

use gpui_kit::{Keystroke, Role};
use serde_json::Value;

use super::{number, options, prop_bool, prop_chosen, prop_str};
use crate::a11y::{self, A11y};
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree};

/// Whether `node` gets a Tab stop from the host.
pub fn takes_stop(node: &Node) -> bool {
    matches!(
        node.node_type,
        NodeType::Slider | NodeType::Rating | NodeType::Tabs | NodeType::Pagination
    ) && !prop_bool(node, "disabled")
}

/// Where a stop stands. A `Tabs` counts its enabled tabs from 0, and stands at -1 when none
/// of them is chosen (so the first key lands on the first tab).
#[derive(Debug, PartialEq)]
struct Position {
    value: f64,
    min: f64,
    max: f64,
    step: f64,
}

/// The value `key` moves `at` to, or `None` for a key the stop does not take.
fn moved(key: &str, at: &Position) -> Option<f64> {
    let value = match key {
        "left" | "down" => at.value - at.step,
        "right" | "up" => at.value + at.step,
        "home" => at.min,
        "end" => at.max,
        _ => return None,
    };
    Some(value.clamp(at.min, at.max))
}

fn prop_f64(node: &Node, key: &str) -> Option<f64> {
    node.props.get(key).and_then(Value::as_f64)
}

/// The defaults and clamps of each component's build (`slider::snapshot`, `basic::rating`,
/// `basic::pagination`); `None` for a `Tabs` with no enabled tab.
fn position(tree: &Tree, node: &Node) -> Option<Position> {
    let (min, max, step, value) = match node.node_type {
        NodeType::Slider => {
            let min = prop_f64(node, "min").unwrap_or(0.);
            let max = prop_f64(node, "max").unwrap_or(100.).max(min);
            let step = prop_f64(node, "step").unwrap_or(1.);
            (min, max, step, prop_f64(node, "value").unwrap_or(min))
        }
        NodeType::Rating => {
            let max = prop_f64(node, "max").unwrap_or(5.).max(1.).trunc();
            (0., max, 1., prop_f64(node, "value").unwrap_or(0.))
        }
        NodeType::Pagination => {
            let max = prop_f64(node, "total").unwrap_or(1.).max(1.).trunc();
            (1., max, 1., prop_f64(node, "page").unwrap_or(1.))
        }
        NodeType::Tabs => {
            let enabled = enabled_tabs(tree, node);
            let max = enabled.len().checked_sub(1)? as f64;
            let current = prop_str(node, "value");
            let at = enabled
                .iter()
                .position(|(value, _)| Some(value.as_str()) == current);
            return Some(Position {
                value: at.map_or(-1., |ix| ix as f64),
                min: 0.,
                max,
                step: 1.,
            });
        }
        _ => return None,
    };
    Some(Position {
        value: value.clamp(min, max),
        min,
        max,
        step,
    })
}

/// The `(value, name)` of each enabled tab of a `Tabs`: its `Tab` children (named by
/// `a11y.label`, then `label`), or its `tabs` entries.
fn enabled_tabs(tree: &Tree, node: &Node) -> Vec<(String, String)> {
    let children: Vec<(String, String, bool)> = node
        .children
        .iter()
        .filter_map(|&child| tree.node(child))
        .filter(|tab| tab.node_type == NodeType::Tab)
        .map(|tab| {
            let value = prop_str(tab, "value").unwrap_or_default().to_owned();
            let name = a11y::of(tab)
                .and_then(|spec| spec.label)
                .or_else(|| prop_str(tab, "label").map(str::to_owned))
                .unwrap_or_else(|| value.clone());
            (value, name, prop_bool(tab, "disabled"))
        })
        .collect();
    let tabs = match children.is_empty() {
        false => children,
        true => options(node, "tabs"),
    };
    tabs.into_iter()
        .filter(|(.., disabled)| !disabled)
        .map(|(value, name, _)| (value, name))
        .collect()
}

/// What the stop reports: the role and value come from the host, and the client's `a11y` only
/// names it (its role would hide what the arrows do). A `Tabs` reads as its chosen tab.
pub fn spec(tree: &Tree, node: &Node, client: Option<A11y>) -> A11y {
    let client = client.unwrap_or_default();
    let mut spec = A11y {
        id: client.id,
        label: client.label,
        description: client.description,
        keyshortcuts: client.keyshortcuts,
        ..A11y::default()
    };
    let at = position(tree, node);
    if node.node_type == NodeType::Tabs {
        spec.role = Some(Role::Tab);
        let chosen = at
            .filter(|at| at.value >= 0.)
            .and_then(|at| enabled_tabs(tree, node).into_iter().nth(at.value as usize));
        if let Some((_, name)) = chosen {
            spec.description = spec.label.replace(name).or(spec.description);
            spec.selected = Some(true);
        }
        return spec;
    }
    spec.role = Some(match node.node_type {
        NodeType::Pagination => Role::SpinButton,
        _ => Role::Slider,
    });
    if let Some(at) = at {
        spec.value = Some(at.value.to_string());
        spec.min = Some(at.min);
        spec.max = Some(at.max);
        spec.step = Some(at.step);
    }
    spec
}

/// Moves the stop `id` for `keystroke` and reports the choice; returns whether the stop took
/// the key (so it goes no further).
pub fn key_down(tree: &mut Tree, id: NodeId, keystroke: &Keystroke) -> bool {
    if keystroke.modifiers.modified() {
        return false;
    }
    let Some(node) = tree.node(id) else {
        return false;
    };
    let Some(at) = position(tree, node) else {
        return false;
    };
    let Some(value) = moved(&keystroke.key, &at) else {
        return false;
    };
    if value == at.value {
        return true;
    }
    let (key, chosen) = match node.node_type {
        NodeType::Slider => ("value", number(value as f32)),
        NodeType::Pagination => ("page", Value::from(value as u64)),
        NodeType::Tabs => {
            let Some((tab, _)) = enabled_tabs(tree, node).into_iter().nth(value as usize) else {
                return true;
            };
            ("value", Value::String(tab))
        }
        _ => ("value", Value::from(value as u64)),
    };
    prop_chosen(tree, id, key, chosen);
    true
}

#[cfg(test)]
mod tests {
    use super::{Position, enabled_tabs, moved, position};
    use crate::schema::NodeType;
    use crate::tree::{Node, Tree};
    use serde_json::{Value, json};

    fn node(node_type: NodeType, props: Value) -> Node {
        Node {
            node_type,
            props: props.as_object().cloned().unwrap_or_default(),
            events: Default::default(),
            children: vec![],
            parent: None,
        }
    }

    #[test]
    fn keys_step_and_stay_in_range() {
        let at = Position {
            value: 9.,
            min: 0.,
            max: 10.,
            step: 2.,
        };
        assert_eq!(moved("right", &at), Some(10.));
        assert_eq!(moved("up", &at), Some(10.));
        assert_eq!(moved("left", &at), Some(7.));
        assert_eq!(moved("down", &at), Some(7.));
        assert_eq!(moved("home", &at), Some(0.));
        assert_eq!(moved("end", &at), Some(10.));
        assert_eq!(moved("a", &at), None);
    }

    #[test]
    fn positions_follow_each_component() {
        let tree = Tree::default();
        let slider = node(
            NodeType::Slider,
            json!({ "value": 150, "max": 120, "step": 5 }),
        );
        assert_eq!(
            position(&tree, &slider),
            Some(Position {
                value: 120.,
                min: 0.,
                max: 120.,
                step: 5.
            })
        );
        let pages = node(NodeType::Pagination, json!({ "total": 4 }));
        assert_eq!(
            position(&tree, &pages),
            Some(Position {
                value: 1.,
                min: 1.,
                max: 4.,
                step: 1.
            })
        );
        let tabs = node(
            NodeType::Tabs,
            json!({ "value": "c", "tabs": [
                { "value": "a", "label": "A" },
                { "value": "b", "label": "B", "disabled": true },
                { "value": "c", "label": "C" },
            ] }),
        );
        assert_eq!(
            enabled_tabs(&tree, &tabs),
            [("a".into(), "A".into()), ("c".into(), "C".into())]
        );
        assert_eq!(
            position(&tree, &tabs),
            Some(Position {
                value: 1.,
                min: 0.,
                max: 1.,
                step: 1.
            })
        );
        assert_eq!(position(&tree, &node(NodeType::Tabs, json!({}))), None);
    }
}
