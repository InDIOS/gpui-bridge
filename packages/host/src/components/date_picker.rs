//! `DatePicker` (PROTOCOL.md section 25): one retained `DatePickerState` per node. The chosen
//! date lives in the tree (`value`, `YYYY-MM-DD`, or `YYYY-MM-DD/YYYY-MM-DD` with `range`), like
//! a Select's value, and `sync` makes the state show it.
//!
//! The crate fixes the mode when the state is built, so `range` is a prop of its own and a
//! change rebuilds the state. A range is reported once both ends are picked.

use std::collections::{HashMap, HashSet};

use gpui_kit::component::Disableable;
use gpui_kit::component::calendar::Date;
use gpui_kit::component::date_picker::{DatePicker, DatePickerEvent, DatePickerState};
use gpui_kit::*;
use serde_json::Value;

use super::calendar::parse_date;
use super::{prop_bool, prop_str, value_chosen};
use crate::Session;
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree};

pub struct DatePickerSlot {
    state: Entity<DatePickerState>,
    /// The mode the state was built with: it is rebuilt when `range` changes.
    range: bool,
    _changes: Subscription,
}

/// Each DatePicker among `members`: the date to show and the mode.
pub struct DatePickerSnapshot(Vec<(NodeId, Date, bool)>);

/// `YYYY-MM-DD`, or with `range` `YYYY-MM-DD/YYYY-MM-DD` with the start not after the end.
pub fn parse(value: &str, range: bool) -> Option<Date> {
    if !range {
        return parse_date(value).map(|day| Date::Single(Some(day)));
    }
    let (start, end) = value.split_once('/')?;
    let (start, end) = (parse_date(start)?, parse_date(end)?);
    (start <= end).then_some(Date::Range(Some(start), Some(end)))
}

pub fn snapshot(tree: &Tree, members: &HashSet<NodeId>) -> DatePickerSnapshot {
    DatePickerSnapshot(
        tree.nodes()
            .filter(|(id, node)| node.node_type == NodeType::DatePicker && members.contains(id))
            .map(|(id, node)| {
                let range = prop_bool(node, "range");
                let empty = if range {
                    Date::Range(None, None)
                } else {
                    Date::Single(None)
                };
                let date = prop_str(node, "value")
                    .and_then(|value| parse(value, range))
                    .unwrap_or(empty);
                (id, date, range)
            })
            .collect(),
    )
}

/// Creates or rebuilds states, drops removed ones, and shows each node's date (which also
/// reverts a controlled DatePicker until the client confirms it).
pub fn sync<V: 'static>(
    slots: &mut HashMap<NodeId, DatePickerSlot>,
    snapshot: DatePickerSnapshot,
    session: &Entity<Session>,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    slots.retain(|id, _| snapshot.0.iter().any(|(picker, ..)| picker == id));
    for (id, date, range) in snapshot.0 {
        if slots.get(&id).is_none_or(|slot| slot.range != range) {
            let state = cx.new(|cx| {
                if range {
                    DatePickerState::range(window, cx)
                } else {
                    DatePickerState::new(window, cx)
                }
            });
            let session = session.clone();
            let _changes = cx.subscribe_in(
                &state,
                window,
                move |_, _, event: &DatePickerEvent, _, cx| {
                    let DatePickerEvent::Change(chosen) = event;
                    let value = match chosen.date() {
                        Date::Single(day) => day.map(|day| day.to_string()),
                        Date::Range(Some(start), Some(end)) => Some(format!("{start}/{end}")),
                        Date::Range(None, None) => None,
                        // Half a range: reported once the other end is picked.
                        Date::Range(..) => return,
                    };
                    session.update(cx, |session, cx| {
                        let current = session
                            .tree
                            .node(id)
                            .and_then(|node| prop_str(node, "value"));
                        if current != value.as_deref() {
                            value_chosen(
                                &mut session.tree,
                                id,
                                value.map_or(Value::Null, Value::String),
                            );
                        }
                        // Synced or not, re-sync: a controlled DatePicker goes back to the client's date.
                        cx.notify();
                    });
                },
            );
            slots.insert(
                id,
                DatePickerSlot {
                    state,
                    range,
                    _changes,
                },
            );
        }
        let slot = &slots[&id];
        let shown = slot.state.read(cx).date();
        // `sync` runs on every render: leave a range the user is halfway through picking alone.
        let picking = matches!(
            shown,
            Date::Range(Some(_), None) | Date::Range(None, Some(_))
        );
        if shown != date && !picking {
            slot.state
                .update(cx, |state, cx| state.set_date(date, window, cx));
        }
    }
}

/// Props: `value` and `range` (applied by `sync`), `placeholder`, `disabled`, `controlled`.
pub fn build(node: &Node, slot: &DatePickerSlot) -> DatePicker {
    let picker = DatePicker::new(&slot.state)
        .cleanable(true)
        .disabled(prop_bool(node, "disabled"));
    match prop_str(node, "placeholder") {
        Some(placeholder) => picker.placeholder(placeholder.to_owned()),
        None => picker,
    }
}
