//! `TimeField` (PROTOCOL.md section 25): one retained `TimeFieldState` per node. The chosen time
//! lives in the tree (`value`, `HH:MM`, or `HH:MM:SS` with `seconds`), and `sync` makes the state
//! show it.
//!
//! The crate fixes the precision when the state is built, so `seconds` is a prop of its own and
//! a change rebuilds the state. The crate has no empty field: without `value` it shows `00:00`.

use std::collections::{HashMap, HashSet};

use chrono::NaiveTime;
use gpui_kit::component::Disableable;
use gpui_kit::component::time_field::{TimeField, TimeFieldEvent, TimeFieldState, TimePrecision};
use gpui_kit::*;
use serde_json::Value;

use super::{prop_bool, prop_str, value_chosen};
use crate::Session;
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree};

pub struct TimeFieldSlot {
    state: Entity<TimeFieldState>,
    /// The precision the state was built with: it is rebuilt when `seconds` changes.
    seconds: bool,
    _changes: Subscription,
}

/// Each TimeField among `members`: the time to show and the precision.
pub struct TimeFieldSnapshot(Vec<(NodeId, NaiveTime, bool)>);

fn format(seconds: bool) -> &'static str {
    if seconds { "%H:%M:%S" } else { "%H:%M" }
}

/// `HH:MM`, or with `seconds` `HH:MM:SS`.
pub fn parse(value: &str, seconds: bool) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(value, format(seconds)).ok()
}

pub fn snapshot(tree: &Tree, members: &HashSet<NodeId>) -> TimeFieldSnapshot {
    TimeFieldSnapshot(
        tree.nodes()
            .filter(|(id, node)| node.node_type == NodeType::TimeField && members.contains(id))
            .map(|(id, node)| {
                let seconds = prop_bool(node, "seconds");
                let time = prop_str(node, "value")
                    .and_then(|value| parse(value, seconds))
                    .unwrap_or_default();
                (id, time, seconds)
            })
            .collect(),
    )
}

/// Creates or rebuilds states, drops removed ones, and shows each node's time (which also
/// reverts a controlled TimeField until the client confirms it).
pub fn sync<V: 'static>(
    slots: &mut HashMap<NodeId, TimeFieldSlot>,
    snapshot: TimeFieldSnapshot,
    session: &Entity<Session>,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    slots.retain(|id, _| snapshot.0.iter().any(|(field, ..)| field == id));
    for (id, time, seconds) in snapshot.0 {
        if slots.get(&id).is_none_or(|slot| slot.seconds != seconds) {
            let precision = if seconds {
                TimePrecision::Second
            } else {
                TimePrecision::Minute
            };
            let state = cx.new(|cx| TimeFieldState::new(window, cx).precision(precision));
            let session = session.clone();
            let _changes = cx.subscribe_in(
                &state,
                window,
                move |_, _, event: &TimeFieldEvent, _, cx| {
                    let TimeFieldEvent::Change(chosen) = event;
                    let value = chosen.format(format(seconds)).to_string();
                    session.update(cx, |session, cx| {
                        let current = session
                            .tree
                            .node(id)
                            .and_then(|node| prop_str(node, "value"));
                        if current != Some(value.as_str()) {
                            value_chosen(&mut session.tree, id, Value::String(value));
                        }
                        // Synced or not, re-sync: a controlled TimeField goes back to the client's time.
                        cx.notify();
                    });
                },
            );
            slots.insert(
                id,
                TimeFieldSlot {
                    state,
                    seconds,
                    _changes,
                },
            );
        }
        let slot = &slots[&id];
        if slot.state.read(cx).time() != time {
            slot.state
                .update(cx, |state, cx| state.set_time(time, window, cx));
        }
    }
}

/// Props: `value` and `seconds` (applied by `sync`), `disabled`, `controlled`.
pub fn build(node: &Node, slot: &TimeFieldSlot) -> TimeField {
    TimeField::new(&slot.state).disabled(prop_bool(node, "disabled"))
}
