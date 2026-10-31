//! The two overlays drawn where they sit in the tree (PROTOCOL.md section 24): a `Popover` and a
//! `HoverCard` are a trigger (the first child) and a panel (the rest), and their open state is
//! synchronized like a `Checkbox`'s. The other two, `AlertDialog` and `Sheet`, are window layers
//! (`render::dialog`, `render::sheet`) and only keep their state here.

use std::sync::Once;
use std::time::Duration;

use gpui_kit::component::Selectable;
use gpui_kit::component::hover_card::HoverCard;
use gpui_kit::component::popover::Popover;
use gpui_kit::*;
use serde_json::Value;

use super::{emit, emit_change, prop_bool, prop_str};
use crate::Session;
use crate::protocol::NodeId;
use crate::tree::{Node, Tree};

/// A `Popover`: its first child is the trigger and the rest the panel. Without a trigger the
/// crate draws nothing, which is warned about once.
pub fn popover(
    id: NodeId,
    node: &Node,
    children: Vec<AnyElement>,
    trigger_style: StyleRefinement,
    session: &Entity<Session>,
) -> Popover {
    let mut children = children.into_iter();
    let popover = Popover::new(("node", id))
        .anchor(anchor(node).unwrap_or(Anchor::TopLeft))
        .arrow(prop_bool(node, "arrow"))
        .open(prop_bool(node, "open"))
        .trigger_style(trigger_style)
        .on_open_change(open_change(id, session.clone()));
    match children.next() {
        Some(trigger) => popover.trigger(Trigger(trigger)).children(children),
        None => {
            warn_no_trigger("Popover");
            popover
        }
    }
}

/// A `HoverCard`: the same shape as a `Popover`, opened and closed by the mouse alone. It has no
/// `open` prop and no `simulate`: the crate owns that state.
pub fn hover_card(
    id: NodeId,
    node: &Node,
    children: Vec<AnyElement>,
    trigger_style: StyleRefinement,
    session: &Entity<Session>,
) -> HoverCard {
    let mut children = children.into_iter();
    let mut card = HoverCard::new(("node", id))
        .anchor(anchor(node).unwrap_or(Anchor::TopCenter))
        .trigger_style(trigger_style)
        .on_open_change(open_change(id, session.clone()));
    if let Some(delay) = delay(node, "openDelay") {
        card = card.open_delay(delay);
    }
    if let Some(delay) = delay(node, "closeDelay") {
        card = card.close_delay(delay);
    }
    match children.next() {
        Some(trigger) => card.trigger(trigger).children(children),
        None => {
            warn_no_trigger("HoverCard");
            card
        }
    }
}

/// The user opened or closed a `Popover`: it keeps the state, as a synced control does, and
/// reports it with `change`. Returns whether the host state changed.
pub fn open_changed(tree: &mut Tree, id: NodeId, open: bool) -> bool {
    let changed = tree.node(id).and_then(|node| node.props.get("open")) != Some(&Value::Bool(open));
    tree.set_host_prop(id, "open", Value::Bool(open));
    emit_change(tree, id, None, Some(open));
    changed
}

/// The user closed an open `Sheet` (its X, Escape or a click on the overlay): it closes and
/// `open` becomes `false` in the host, as when a `Dialog` is answered.
pub fn sheet_closed(tree: &mut Tree, id: NodeId) {
    tree.set_host_prop(id, "open", Value::Bool(false));
    emit(tree, id, "close", None, None);
}

/// The `open` and `close` half of the crate's callback: `change` is opt-in, the state is not.
fn open_change(id: NodeId, session: Entity<Session>) -> impl Fn(&bool, &mut Window, &mut App) {
    move |open, _, cx| {
        session.update(cx, |session, cx| {
            if open_changed(&mut session.tree, id, *open) {
                cx.notify();
            }
        });
    }
}

/// The `anchor` prop as a GPUI anchor. Each node has its own default (`top-left` for a `Popover`,
/// `top-center` for a `HoverCard`).
fn anchor(node: &Node) -> Option<Anchor> {
    Some(match prop_str(node, "anchor")? {
        "top-left" => Anchor::TopLeft,
        "top-center" => Anchor::TopCenter,
        "top-right" => Anchor::TopRight,
        "left-center" => Anchor::LeftCenter,
        "right-center" => Anchor::RightCenter,
        "bottom-left" => Anchor::BottomLeft,
        "bottom-center" => Anchor::BottomCenter,
        _ => Anchor::BottomRight,
    })
}

/// A delay in milliseconds. `from_millis` saturates instead of panicking on the huge number the
/// schema accepts (`from_secs_f32` is what an oversized `Shimmer` duration killed the host with).
fn delay(node: &Node, key: &str) -> Option<Duration> {
    node.props
        .get(key)
        .and_then(Value::as_f64)
        .map(|ms| Duration::from_millis(ms.max(0.) as u64))
}

/// Warns once that an overlay without children draws nothing, and why.
fn warn_no_trigger(name: &str) {
    static POPOVER: Once = Once::new();
    static HOVER_CARD: Once = Once::new();
    let warned = match name {
        "Popover" => &POPOVER,
        _ => &HOVER_CARD,
    };
    warned.call_once(|| {
        log::warn!("'{name}' without children draws nothing: the first child is its trigger")
    });
}

/// The trigger slot of a `Popover`. The crate asks a trigger to mark itself open, which is a look
/// of its own components: this wrapper has none, so the child keeps the one it has.
struct Trigger(AnyElement);

impl IntoElement for Trigger {
    /// The child itself, so its own look is the one that is drawn (see `Trigger`).
    type Element = AnyElement;

    fn into_element(self) -> Self::Element {
        self.0
    }
}

impl Selectable for Trigger {
    fn selected(self, _: bool) -> Self {
        self
    }

    fn is_selected(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    // Not `super::*`: gpui's own `test` attribute would shadow the standard one.
    use super::{anchor, delay};
    use crate::schema::NodeType;
    use crate::tree::Node;
    use gpui_kit::Anchor;
    use serde_json::{Value, json};

    fn node(props: Value) -> Node {
        Node {
            node_type: NodeType::HoverCard,
            props: props.as_object().cloned().unwrap_or_default(),
            events: Default::default(),
            children: vec![],
            parent: None,
        }
    }

    #[test]
    fn every_anchor_names_its_corner() {
        for (name, corner) in [
            ("top-left", Anchor::TopLeft),
            ("top-center", Anchor::TopCenter),
            ("top-right", Anchor::TopRight),
            ("left-center", Anchor::LeftCenter),
            ("right-center", Anchor::RightCenter),
            ("bottom-left", Anchor::BottomLeft),
            ("bottom-center", Anchor::BottomCenter),
            ("bottom-right", Anchor::BottomRight),
        ] {
            assert_eq!(
                anchor(&node(json!({ "anchor": name }))),
                Some(corner),
                "{name}"
            );
        }
        assert_eq!(anchor(&node(json!({}))), None);
    }

    /// The schema accepts any finite number, so a huge delay reaches `Duration::from_millis`.
    #[test]
    fn a_huge_delay_saturates_instead_of_panicking() {
        let delay = delay(&node(json!({ "openDelay": 1e300 })), "openDelay").unwrap();
        assert!(delay > std::time::Duration::from_secs(3_600), "{delay:?}");
    }
}
