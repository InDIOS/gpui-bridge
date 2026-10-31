//! `Combobox` (PROTOCOL.md section 25): one retained `ComboboxState` per node, always with
//! several choices. The chosen values live in the tree (`value`, a list of option values), like
//! a Select's value, and `sync` makes the state show them. The trigger is the crate's own, drawn
//! from `options`, `value` and `placeholder`: nothing of it comes from the client.

use std::collections::{HashMap, HashSet};

use gpui_kit::component::combobox::{Combobox, ComboboxEvent, ComboboxState};
use gpui_kit::component::select::SearchableVec;
use gpui_kit::*;
use serde_json::Value;

use super::select::{Choice, Options, choices};
use super::{options, prop_bool, prop_str, value_chosen};
use crate::Session;
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree};

type State = ComboboxState<SearchableVec<Choice>>;

pub struct ComboboxSlot {
    state: Entity<State>,
    /// What the state was built from: it is rebuilt when they change.
    options: Options,
    searchable: bool,
    _changes: Subscription,
}

/// Each Combobox among `members`: options, `searchable` and the values to show, sorted.
pub struct ComboboxSnapshot(Vec<(NodeId, Options, bool, Vec<String>)>);

/// A node's `value` as strings.
fn values(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect()
}

pub fn snapshot(tree: &Tree, members: &HashSet<NodeId>) -> ComboboxSnapshot {
    ComboboxSnapshot(
        tree.nodes()
            .filter(|(id, node)| node.node_type == NodeType::Combobox && members.contains(id))
            .map(|(id, node)| {
                let options = options(node, "options");
                let chosen = values(node.props.get("value"));
                // Only known options, once each: the state drops the rest, and `sync` would
                // otherwise set them again on every render.
                let mut shown: Vec<String> = options
                    .iter()
                    .filter(|(option, ..)| chosen.contains(option))
                    .map(|(option, ..)| option.clone())
                    .collect();
                shown.sort();
                shown.dedup();
                (id, options, prop_bool(node, "searchable"), shown)
            })
            .collect(),
    )
}

/// Creates or rebuilds states, drops removed ones, and shows each node's values (which also
/// reverts a controlled Combobox until the client confirms them).
pub fn sync<V: 'static>(
    slots: &mut HashMap<NodeId, ComboboxSlot>,
    snapshot: ComboboxSnapshot,
    session: &Entity<Session>,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    slots.retain(|id, _| snapshot.0.iter().any(|(combobox, ..)| combobox == id));
    for (id, options, searchable, value) in snapshot.0 {
        let stale = slots
            .get(&id)
            .is_none_or(|slot| slot.options != options || slot.searchable != searchable);
        if stale {
            let state = cx.new(|cx| {
                State::new(choices(&options), vec![], window, cx)
                    .multiple(true)
                    .searchable(searchable)
            });
            let session = session.clone();
            let _changes = cx.subscribe_in(
                &state,
                window,
                move |_, _, event: &ComboboxEvent<SearchableVec<Choice>>, _, cx| {
                    let ComboboxEvent::Change(chosen) = event else {
                        return;
                    };
                    session.update(cx, |session, cx| {
                        let current = values(
                            session
                                .tree
                                .node(id)
                                .and_then(|node| node.props.get("value")),
                        );
                        if current != *chosen {
                            let chosen = chosen.iter().cloned().map(Value::String).collect();
                            value_chosen(&mut session.tree, id, Value::Array(chosen));
                        }
                        // Synced or not, re-sync: a controlled Combobox goes back to the client's values.
                        cx.notify();
                    });
                },
            );
            slots.insert(
                id,
                ComboboxSlot {
                    state,
                    options,
                    searchable,
                    _changes,
                },
            );
        }
        let slot = &slots[&id];
        let mut shown = slot.state.read(cx).selected_values();
        shown.sort();
        if shown != value {
            slot.state.update(cx, |state, cx| {
                state.set_selected_values(&value, window, cx)
            });
        }
    }
}

/// Props: `options`, `value` and `searchable` (applied by `sync`), `placeholder`, `disabled`,
/// `controlled`.
pub fn build(node: &Node, slot: &ComboboxSlot) -> Combobox<SearchableVec<Choice>> {
    let combobox = Combobox::new(&slot.state).disabled(prop_bool(node, "disabled"));
    match prop_str(node, "placeholder") {
        Some(placeholder) => combobox.placeholder(placeholder.to_owned()),
        None => combobox,
    }
}
