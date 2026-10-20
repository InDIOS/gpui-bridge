//! `Calendar` (PROTOCOL.md section 17): one `CalendarState` per node. The chosen day lives in
//! the tree (`value`, `YYYY-MM-DD`), like a Select's value, and `sync` makes the state show it.

use std::collections::{HashMap, HashSet};

use chrono::{NaiveDate, Weekday};
use gpui_kit::component::calendar::{Calendar, CalendarEvent, CalendarState, Date};
use gpui_kit::*;

use super::value_chosen;
use crate::Session;
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree};

pub struct CalendarSlot {
    state: Entity<CalendarState>,
    _changes: Subscription,
}

impl CalendarSlot {
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        let handle = self.state.read(cx).focus_handle.clone();
        handle.focus(window, cx);
    }
}

/// Each Calendar among `members`: the day to show and how many months.
pub struct CalendarSnapshot(Vec<(NodeId, Option<NaiveDate>, usize)>);

/// A `value` of the form `YYYY-MM-DD`; anything else is no day.
pub fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

pub fn snapshot(tree: &Tree, members: &HashSet<NodeId>) -> CalendarSnapshot {
    CalendarSnapshot(
        tree.nodes()
            .filter(|(id, node)| node.node_type == NodeType::Calendar && members.contains(id))
            .map(|(id, node)| {
                let value = node
                    .props
                    .get("value")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                let months = node
                    .props
                    .get("months")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(1)
                    .clamp(1, 3) as usize;
                (id, parse_date(value), months)
            })
            .collect(),
    )
}

/// Creates states, drops removed ones, and shows each node's day (which also reverts a
/// controlled Calendar until the client confirms it).
pub fn sync<V: 'static>(
    slots: &mut HashMap<NodeId, CalendarSlot>,
    snapshot: CalendarSnapshot,
    session: &Entity<Session>,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    slots.retain(|id, _| snapshot.0.iter().any(|(calendar, ..)| calendar == id));
    for (id, date, months) in snapshot.0 {
        let state = slots.entry(id).or_insert_with(|| {
            let state = cx.new(|cx| {
                let mut state = CalendarState::new(window, cx);
                state.set_number_of_months(months, window, cx);
                state
            });
            let session = session.clone();
            let _changes =
                cx.subscribe_in(&state, window, move |_, _, event: &CalendarEvent, _, cx| {
                    let CalendarEvent::Selected(chosen) = event;
                    let value = chosen
                        .start()
                        .map(|date| date.to_string())
                        .unwrap_or_default();
                    session.update(cx, |session, cx| {
                        let current = session
                            .tree
                            .node(id)
                            .and_then(|node| node.props.get("value"))
                            .and_then(serde_json::Value::as_str);
                        if current != Some(value.as_str()) {
                            value_chosen(&mut session.tree, id, serde_json::Value::String(value));
                        }
                        // Synced or not, re-sync: a controlled Calendar goes back to the client's day.
                        cx.notify();
                    });
                });
            CalendarSlot { state, _changes }
        });
        if state.state.read(cx).number_of_months() != months {
            state.state.update(cx, |state, cx| {
                state.set_number_of_months(months, window, cx)
            });
        }
        let shown = state.state.read(cx).date().start();
        if shown != date {
            state.state.update(cx, |state, cx| {
                state.set_date(Date::Single(date), window, cx)
            });
        }
    }
}

/// Props: `value` (applied by `sync`), `months`, `weekStartsOn` (sunday or monday), `disabled`.
pub fn build(node: &Node, slot: &CalendarSlot) -> Calendar {
    let months = node
        .props
        .get("months")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(1)
        .clamp(1, 3) as usize;
    let first = match node
        .props
        .get("weekStartsOn")
        .and_then(serde_json::Value::as_str)
    {
        Some("monday") => Weekday::Mon,
        _ => Weekday::Sun,
    };
    Calendar::new(&slot.state)
        .number_of_months(months)
        .first_day_of_week(first)
}
