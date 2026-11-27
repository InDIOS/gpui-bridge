//! A Tab stop and arrow keys for the components gpui-component leaves without them: `Rating`
//! and `Pagination` (PROTOCOL.md section 16). A key makes the choice a click would, so the
//! synced and controlled modes work as they do with the pointer. Since 0.10 the crate gives
//! `Slider` and `Tabs` their own stop and keys, so the host leaves them alone.

use gpui_kit::{Keystroke, Role};
use serde_json::Value;

use super::{prop_bool, prop_chosen};
use crate::a11y::A11y;
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree};

/// Whether `node` gets a Tab stop from the host.
pub fn takes_stop(node: &Node) -> bool {
    matches!(node.node_type, NodeType::Rating | NodeType::Pagination)
        && !prop_bool(node, "disabled")
}

/// Where a stop stands.
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

/// The defaults and clamps of each component's build (`basic::rating`, `basic::pagination`).
fn position(node: &Node) -> Option<Position> {
    let (min, max, value) = match node.node_type {
        NodeType::Rating => {
            let max = prop_f64(node, "max").unwrap_or(5.).max(1.).trunc();
            (0., max, prop_f64(node, "value").unwrap_or(0.))
        }
        NodeType::Pagination => {
            let max = prop_f64(node, "total").unwrap_or(1.).max(1.).trunc();
            (1., max, prop_f64(node, "page").unwrap_or(1.))
        }
        _ => return None,
    };
    Some(Position {
        value: value.clamp(min, max),
        min,
        max,
        step: 1.,
    })
}

/// What the stop reports: the role and value come from the host, and the client's `a11y` only
/// names it (its role would hide what the arrows do).
pub fn spec(node: &Node, client: Option<A11y>) -> A11y {
    let client = client.unwrap_or_default();
    let mut spec = A11y {
        id: client.id,
        label: client.label,
        description: client.description,
        keyshortcuts: client.keyshortcuts,
        ..A11y::default()
    };
    spec.role = Some(match node.node_type {
        NodeType::Pagination => Role::SpinButton,
        _ => Role::Slider,
    });
    if let Some(at) = position(node) {
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
    let Some(at) = position(node) else {
        return false;
    };
    let Some(value) = moved(&keystroke.key, &at) else {
        return false;
    };
    if value == at.value {
        return true;
    }
    let key = match node.node_type {
        NodeType::Pagination => "page",
        _ => "value",
    };
    prop_chosen(tree, id, key, Value::from(value as u64));
    true
}

#[cfg(test)]
mod tests {
    use super::{Position, moved, position};
    use crate::schema::NodeType;
    use crate::tree::Node;
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
        let stars = node(NodeType::Rating, json!({ "value": 9, "max": 4.5 }));
        assert_eq!(
            position(&stars),
            Some(Position {
                value: 4.,
                min: 0.,
                max: 4.,
                step: 1.
            })
        );
        let pages = node(NodeType::Pagination, json!({ "total": 4 }));
        assert_eq!(
            position(&pages),
            Some(Position {
                value: 1.,
                min: 1.,
                max: 4.,
                step: 1.
            })
        );
        assert_eq!(position(&node(NodeType::Slider, json!({}))), None);
    }
}
