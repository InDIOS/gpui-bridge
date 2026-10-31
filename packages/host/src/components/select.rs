//! `Select`: one retained `SelectState` per node. The chosen value lives in the tree (`value`),
//! like `checked` for a Checkbox, and `sync` makes the state show it.

use std::collections::{HashMap, HashSet};

use gpui_kit::component::select::{SearchableVec, Select, SelectEvent, SelectItem, SelectState};
use gpui_kit::*;
use serde_json::Value;

use super::{options, prop_bool, prop_str, value_chosen};
use crate::Session;
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree};

#[derive(Clone)]
pub struct Choice {
    value: String,
    label: SharedString,
    disabled: bool,
}

impl SelectItem for Choice {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &String {
        &self.value
    }

    fn disabled(&self) -> bool {
        self.disabled
    }
}

type State = SelectState<SearchableVec<Choice>>;
pub type Options = Vec<(String, String, bool)>;

/// The list a Select or a Combobox shows for `options`.
pub fn choices(options: &Options) -> SearchableVec<Choice> {
    SearchableVec::new(
        options
            .iter()
            .map(|(value, label, disabled)| Choice {
                value: value.clone(),
                label: label.clone().into(),
                disabled: *disabled,
            })
            .collect::<Vec<_>>(),
    )
}

pub struct SelectSlot {
    state: Entity<State>,
    /// What the state was built from: it is rebuilt when they change.
    options: Options,
    searchable: bool,
    _confirm: Subscription,
}

/// Each Select among `members`: options, `searchable` and the value to show.
pub struct SelectSnapshot(Vec<(NodeId, Options, bool, Option<String>)>);

pub fn snapshot(tree: &Tree, members: &HashSet<NodeId>) -> SelectSnapshot {
    SelectSnapshot(
        tree.nodes()
            .filter(|(id, node)| node.node_type == NodeType::Select && members.contains(id))
            .map(|(id, node)| {
                (
                    id,
                    options(node, "options"),
                    prop_bool(node, "searchable"),
                    prop_str(node, "value").map(str::to_owned),
                )
            })
            .collect(),
    )
}

/// Creates or rebuilds states, drops removed ones, and shows each node's value (which also
/// reverts a controlled Select's own selection until the client confirms it).
pub fn sync<V: 'static>(
    slots: &mut HashMap<NodeId, SelectSlot>,
    snapshot: SelectSnapshot,
    session: &Entity<Session>,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    slots.retain(|id, _| snapshot.0.iter().any(|(select, ..)| select == id));
    for (id, options, searchable, value) in snapshot.0 {
        let stale = slots
            .get(&id)
            .is_none_or(|slot| slot.options != options || slot.searchable != searchable);
        if stale {
            let state =
                cx.new(|cx| State::new(choices(&options), None, window, cx).searchable(searchable));
            let session = session.clone();
            let _confirm = cx.subscribe_in(
                &state,
                window,
                move |_, _, event: &SelectEvent<SearchableVec<Choice>>, _, cx| {
                    let SelectEvent::Confirm(Some(chosen)) = event else {
                        return;
                    };
                    session.update(cx, |session, cx| {
                        let current = session
                            .tree
                            .node(id)
                            .and_then(|node| prop_str(node, "value"));
                        if current != Some(chosen.as_str()) {
                            value_chosen(&mut session.tree, id, Value::String(chosen.clone()));
                        }
                        // Synced or not, re-sync: a controlled Select goes back to the client's value.
                        cx.notify();
                    });
                },
            );
            slots.insert(
                id,
                SelectSlot {
                    state,
                    options,
                    searchable,
                    _confirm,
                },
            );
        }
        let slot = &slots[&id];
        if slot.state.read(cx).selected_value() != value.as_ref() {
            slot.state.update(cx, |state, cx| match &value {
                Some(value) => state.set_selected_value(value, window, cx),
                None => state.set_selected_index(None, window, cx),
            });
        }
    }
}

/// Props: `options` and `value` (applied by `sync`), `placeholder`, `searchable`, `disabled`, `controlled`.
pub fn build(node: &Node, slot: &SelectSlot) -> Select<SearchableVec<Choice>> {
    let select = Select::new(&slot.state).disabled(prop_bool(node, "disabled"));
    match prop_str(node, "placeholder") {
        Some(placeholder) => select.placeholder(placeholder.to_owned()),
        None => select,
    }
}
