//! `Carousel` (PROTOCOL.md section 18): one retained `CarouselState` per node. The slides are
//! the node's children, one per item; `value` is the selected index.

use std::collections::{HashMap, HashSet};

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::carousel::{
    Carousel, CarouselContent, CarouselEvent, CarouselItem, CarouselPagination,
    CarouselPaginationItem, CarouselState,
};
use gpui_kit::component::{Icon, h_flex};
use gpui_kit::*;
use serde_json::Value;

use super::{icon_path, value_chosen};
use crate::Session;
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree};

pub struct CarouselSlot {
    state: Entity<CarouselState>,
    /// How many dots `build` draws (the state has the same count).
    item_count: usize,
    _events: Subscription,
}

/// Each Carousel among `members`: how many slides, the axis and looping, and the selected index.
pub struct CarouselSnapshot(Vec<(NodeId, usize, Axis, bool, Option<usize>)>);

fn axis(node: &Node) -> Axis {
    match node.props.get("axis").and_then(Value::as_str) {
        Some("vertical") => Axis::Vertical,
        _ => Axis::Horizontal,
    }
}

pub fn snapshot(tree: &Tree, members: &HashSet<NodeId>) -> CarouselSnapshot {
    CarouselSnapshot(
        tree.nodes()
            .filter(|(id, node)| node.node_type == NodeType::Carousel && members.contains(id))
            .map(|(id, node)| {
                let selected = node
                    .props
                    .get("value")
                    .and_then(Value::as_u64)
                    .map(|index| index as usize);
                let looping = node
                    .props
                    .get("looping")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                (id, node.children.len(), axis(node), looping, selected)
            })
            .collect(),
    )
}

/// Creates state, drops removed ones, and shows each node's selected index (which also reverts a
/// controlled Carousel until the client confirms it).
pub fn sync<V: 'static>(
    slots: &mut HashMap<NodeId, CarouselSlot>,
    snapshot: CarouselSnapshot,
    session: &Entity<Session>,
    cx: &mut Context<V>,
) {
    slots.retain(|id, _| snapshot.0.iter().any(|(carousel, ..)| carousel == id));
    for (id, count, axis, looping, selected) in snapshot.0 {
        match slots.get(&id) {
            Some(slot) => {
                let state = slot.state.clone();
                state.update(cx, |state, cx| {
                    state.set_item_count(count, cx);
                    state.set_axis(axis, cx);
                    state.set_looping(looping, cx);
                    if let Some(index) = selected
                        && state.selected_index() != Some(index)
                    {
                        state.set_selected_index(index, cx);
                    }
                });
                if let Some(slot) = slots.get_mut(&id) {
                    slot.item_count = count;
                }
            }
            None => {
                let state = cx.new(|_| {
                    let state = CarouselState::new(count)
                        .with_axis(axis)
                        .with_looping(looping);
                    match selected {
                        Some(index) => state.with_selected_index(index),
                        None => state,
                    }
                });
                let session = session.clone();
                let events = cx.subscribe(&state, move |_, _, event: &CarouselEvent, cx| {
                    let CarouselEvent::Change(index) = event;
                    let value = Value::from(*index as u64);
                    session.update(cx, |session, cx| {
                        let current = session
                            .tree
                            .node(id)
                            .and_then(|node| node.props.get("value"))
                            .and_then(Value::as_u64);
                        if current != Some(*index as u64) {
                            value_chosen(&mut session.tree, id, value);
                        }
                        // Synced or not, re-sync: a controlled Carousel goes back to the client's index.
                        cx.notify();
                    });
                });
                slots.insert(
                    id,
                    CarouselSlot {
                        state,
                        item_count: count,
                        _events: events,
                    },
                );
            }
        }
    }
}

/// A previous/next button. `CarouselPrevious`/`CarouselNext` position themselves absolutely over
/// the content, which drew them outside the visible box here; an ordinary row is predictable and
/// still keeps the keyboard reachable through the carousel.
fn arrow(id: NodeId, state: &Entity<CarouselState>, next: bool) -> Button {
    let (name, icon) = if next {
        ("next", "chevron-right")
    } else {
        ("previous", "chevron-left")
    };
    let state = state.clone();
    Button::new(ElementId::Name(format!("carousel-{id}-{name}").into()))
        .ghost()
        .icon(Icon::default().path(icon_path(icon)))
        .on_click(move |_, _, cx| {
            state.update(cx, |state, cx| {
                if next {
                    state.select_next(cx);
                } else {
                    state.select_previous(cx);
                }
            });
        })
}

/// Props: `value` (applied by `sync`), `axis`, `looping`, `controls` (default true: previous and
/// next buttons), `pagination` (default false) and `controlled`. Its children are the slides;
/// `change` carries the selected index.
pub fn build(id: NodeId, node: &Node, slot: &CarouselSlot, slides: Vec<AnyElement>) -> Carousel {
    let flag = |key, default| {
        node.props
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(default)
    };
    let items: Vec<AnyElement> = slides
        .into_iter()
        .enumerate()
        .map(|(index, slide)| {
            CarouselItem::new(
                ElementId::Name(format!("carousel-{id}-item-{index}").into()),
                index,
                &slot.state,
            )
            .child(slide)
            .into_any_element()
        })
        .collect();
    // The root fills the node's box (the caller gives it a height with `class`) and the content
    // takes what the controls leave: a slide sized `size-full` needs a definite height.
    let mut carousel = Carousel::new(("carousel", id), &slot.state).h_full().child(
        CarouselContent::new(&slot.state)
            .flex_1()
            .min_h_0()
            .children(items),
    );
    if flag("controls", true) {
        carousel = carousel.child(
            h_flex()
                .justify_center()
                .gap_2()
                .child(arrow(id, &slot.state, false))
                .child(arrow(id, &slot.state, true)),
        );
    }
    if flag("pagination", false) {
        let dots = (0..slot.item_count).map(|index| {
            CarouselPaginationItem::new(
                ElementId::Name(format!("carousel-{id}-dot-{index}").into()),
                index,
                &slot.state,
            )
        });
        carousel = carousel.child(CarouselPagination::new().children(dots));
    }
    carousel
}
