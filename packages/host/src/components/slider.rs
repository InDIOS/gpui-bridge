//! `Slider`: one retained `SliderState` per node. The value lives in the tree (`value`), and
//! `sync` makes the state show it.

use std::collections::{HashMap, HashSet};

use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::*;

use super::{number, prop_bool, prop_f32, value_chosen};
use crate::Session;
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree};

/// `min`, `max`, `step`.
type Range = (f32, f32, f32);

pub struct SliderSlot {
    state: Entity<SliderState>,
    range: Range,
    _changes: Subscription,
}

/// Each Slider among `members`: its range and the value to show.
pub struct SliderSnapshot(Vec<(NodeId, Range, f32)>);

pub fn snapshot(tree: &Tree, members: &HashSet<NodeId>) -> SliderSnapshot {
    SliderSnapshot(
        tree.nodes()
            .filter(|(id, node)| node.node_type == NodeType::Slider && members.contains(id))
            .map(|(id, node)| {
                let min = prop_f32(node, "min").unwrap_or(0.);
                // A `max` not above `min` would divide by zero in the thumb position.
                let max = prop_f32(node, "max")
                    .unwrap_or(100.)
                    .max(min + f32::EPSILON);
                let step = prop_f32(node, "step").unwrap_or(1.);
                let value = prop_f32(node, "value").unwrap_or(min).clamp(min, max);
                (id, (min, max, step), value)
            })
            .collect(),
    )
}

/// Creates states, applies range changes, drops removed ones, and shows each node's value
/// (which also reverts a controlled Slider until the client confirms the drag).
pub fn sync<V: 'static>(
    slots: &mut HashMap<NodeId, SliderSlot>,
    snapshot: SliderSnapshot,
    session: &Entity<Session>,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    slots.retain(|id, _| snapshot.0.iter().any(|(slider, ..)| slider == id));
    for (id, range, value) in snapshot.0 {
        let (min, max, step) = range;
        let slot = slots.entry(id).or_insert_with(|| {
            let state = cx.new(|_| {
                SliderState::new()
                    .min(min)
                    .max(max)
                    .step(step)
                    .default_value(value)
            });
            let session = session.clone();
            let _changes =
                cx.subscribe_in(&state, window, move |_, _, event: &SliderEvent, _, cx| {
                    let SliderEvent::Change(chosen) = event else {
                        return;
                    };
                    let chosen = chosen.start();
                    session.update(cx, |session, cx| {
                        let current = session
                            .tree
                            .node(id)
                            .and_then(|node| prop_f32(node, "value"));
                        if current != Some(chosen) {
                            value_chosen(&mut session.tree, id, number(chosen));
                        }
                        cx.notify();
                    });
                });
            SliderSlot {
                state,
                range,
                _changes,
            }
        });
        if slot.range != range {
            slot.range = range;
            // The range is set through builder methods only.
            slot.state.update(cx, |state, _| {
                *state = std::mem::replace(state, SliderState::new())
                    .min(min)
                    .max(max)
                    .step(step);
            });
        }
        if slot.state.read(cx).value().start() != value {
            slot.state
                .update(cx, |state, cx| state.set_value(value, window, cx));
        }
    }
}

/// Props: `value`, `min`, `max`, `step` (applied by `sync`), `disabled`, `controlled`.
pub fn build(node: &Node, slot: &SliderSlot) -> Slider {
    Slider::new(&slot.state).disabled(prop_bool(node, "disabled"))
}
