//! `List` (PROTOCOL.md section 27): gpui-component's list over the node's `ListItem` children.
//! One retained `ListState` per node; its delegate reads the rows from the tree when it draws
//! them, and the selection follows `value`, as a Table's follows `selected`.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui_kit::component::list::{List, ListDelegate, ListEvent, ListItem, ListState};
use gpui_kit::component::{IndexPath, h_flex};
use gpui_kit::*;
use serde_json::Value;

use super::{chosen, emit, prop_bool, prop_str};
use crate::Session;
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree};

/// Draws the content of one row (a `ListItem`'s children) in the list's own pass.
pub type RenderItem = Rc<dyn Fn(NodeId, &mut Window, &mut App) -> Vec<AnyElement>>;

/// The crate's default: `loadMore` is sent when the last rows in view are this close to the end.
const THRESHOLD: usize = 20;

pub struct Delegate {
    session: Entity<Session>,
    id: NodeId,
    render: RenderItem,
    /// `loadMoreThreshold`, kept here because the crate asks for it without an `App`.
    threshold: usize,
    /// The row count `loadMore` was last sent for: the crate asks on every frame near the end.
    asked: Option<usize>,
}

impl ListDelegate for Delegate {
    type Item = ListItem;

    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        searched(&self.session.read(cx).tree, self.id, query);
        Task::ready(())
    }

    fn items_count(&self, _: usize, cx: &App) -> usize {
        items(&self.session.read(cx).tree, self.id).count()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<ListItem> {
        let tree = &self.session.read(cx).tree;
        let (item, node) = items(tree, self.id).nth(ix.row)?;
        let disabled = prop_bool(node, "disabled");
        let label = crate::a11y::of(node).and_then(|spec| spec.label);
        let row = ListItem::new(("list-item", item)).disabled(disabled);
        let row = match label {
            Some(label) => row.accessibility_label(label),
            None => row,
        };
        // The crate stacks its children in a block; a row of them reads like a list entry.
        let content = (self.render)(item, window, cx);
        Some(row.child(h_flex().w_full().gap_2().items_center().children(content)))
    }

    // The selection is the List's `value`, kept in the tree by `list_event`.
    fn set_selected_index(
        &mut self,
        _: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
    }

    fn loading(&self, cx: &App) -> bool {
        self.session
            .read(cx)
            .tree
            .node(self.id)
            .is_some_and(|node| prop_bool(node, "loading"))
    }

    fn has_more(&self, cx: &App) -> bool {
        self.session
            .read(cx)
            .tree
            .node(self.id)
            .is_some_and(|node| prop_bool(node, "hasMore"))
    }

    fn load_more_threshold(&self) -> usize {
        self.threshold
    }

    fn load_more(&mut self, _: &mut Window, cx: &mut Context<ListState<Self>>) {
        let tree = &self.session.read(cx).tree;
        let count = items(tree, self.id).count();
        if self.asked != Some(count) {
            self.asked = Some(count);
            more(tree, self.id);
        }
    }
}

/// A List's rows: its `ListItem` children, in order (other children are not drawn).
pub fn items(tree: &Tree, id: NodeId) -> impl Iterator<Item = (NodeId, &Node)> {
    tree.node(id)
        .into_iter()
        .flat_map(|node| &node.children)
        .filter_map(move |&child| {
            tree.node(child)
                .filter(|node| node.node_type == NodeType::ListItem)
                .map(|node| (child, node))
        })
}

fn item_value(node: &Node) -> &str {
    prop_str(node, "value").unwrap_or_default()
}

/// The row whose `value` is the List's `value` (none: nothing is selected).
fn position(tree: &Tree, id: NodeId) -> Option<usize> {
    let value = tree.node(id).and_then(|node| prop_str(node, "value"))?;
    items(tree, id).position(|(_, item)| item_value(item) == value)
}

/// The row the arrow keys select when they land on `ix`: `ix`, or the next enabled row past it
/// in the direction the selection moved from `from`.
fn enabled_row(disabled: &[bool], ix: usize, from: Option<usize>) -> Option<usize> {
    if from.is_some_and(|from| ix < from) {
        (0..=ix).rev().find(|&row| !disabled[row])
    } else {
        (ix..disabled.len()).find(|&row| !disabled[row])
    }
}

pub struct ListSlot {
    state: Entity<ListState<Delegate>>,
    searchable: bool,
    _events: Subscription,
}

impl ListSlot {
    /// `focus` on a List: the arrow keys then move the selection.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.state.update(cx, |state, cx| state.focus(window, cx));
    }
}

/// Each List among `members`: its selected row, `searchable` and `loadMoreThreshold`.
pub struct ListSnapshot(Vec<(NodeId, Option<usize>, bool, usize)>);

pub fn snapshot(tree: &Tree, members: &HashSet<NodeId>) -> ListSnapshot {
    ListSnapshot(
        tree.nodes()
            .filter(|(id, node)| node.node_type == NodeType::List && members.contains(id))
            .map(|(id, node)| {
                let threshold = node
                    .props
                    .get("loadMoreThreshold")
                    .and_then(Value::as_u64)
                    .map_or(THRESHOLD, |n| n as usize);
                (
                    id,
                    position(tree, id),
                    prop_bool(node, "searchable"),
                    threshold,
                )
            })
            .collect(),
    )
}

/// Creates states, drops removed ones and shows each node's selection (which also reverts a
/// controlled List's own selection until the client confirms it).
pub fn sync<V: 'static>(
    slots: &mut HashMap<NodeId, ListSlot>,
    snapshot: ListSnapshot,
    session: &Entity<Session>,
    render: &RenderItem,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    slots.retain(|id, _| snapshot.0.iter().any(|(list, ..)| list == id));
    for (id, selected, searchable, threshold) in snapshot.0 {
        let slot = slots.entry(id).or_insert_with(|| {
            let delegate = Delegate {
                session: session.clone(),
                id,
                render: render.clone(),
                threshold,
                asked: None,
            };
            let state = cx.new(|cx| ListState::new(delegate, window, cx).searchable(searchable));
            let session = session.clone();
            let _events = cx.subscribe_in(&state, window, move |_, state, event, window, cx| {
                list_event(&session, id, state, event, window, cx);
            });
            ListSlot {
                state,
                searchable,
                _events,
            }
        });
        let changed = slot.searchable != searchable;
        slot.searchable = searchable;
        slot.state.update(cx, |state, cx| {
            if changed {
                state.set_searchable(searchable, cx);
            }
            state.delegate_mut().threshold = threshold;
            if state.selected_index().map(|ix| ix.row) != selected {
                state.set_selected_index(selected.map(IndexPath::new), window, cx);
                state.scroll_to_selected_item(window, cx);
            }
            // Rows may have changed: draw them again.
            cx.notify();
        });
    }
}

/// The user moved, confirmed or cleared the selection in the window.
fn list_event<V: 'static>(
    session: &Entity<Session>,
    id: NodeId,
    state: &Entity<ListState<Delegate>>,
    event: &ListEvent,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    let mut moved = None;
    session.update(cx, |session, cx| {
        let tree = &mut session.tree;
        let current = position(tree, id);
        let rows: Vec<(String, bool)> = items(tree, id)
            .map(|(_, node)| (item_value(node).to_owned(), prop_bool(node, "disabled")))
            .collect();
        match *event {
            // The arrow keys: a disabled row is passed over.
            ListEvent::Select(ix) => {
                let disabled: Vec<bool> = rows.iter().map(|(_, disabled)| *disabled).collect();
                let row = enabled_row(&disabled, ix.row, current).or(current);
                if row != Some(ix.row) {
                    moved = Some(row);
                }
                if let Some(row) = row.filter(|&row| current != Some(row)) {
                    selected(tree, id, Some(&rows[row].0));
                }
            }
            // A click or Enter; on a disabled row it does nothing and the selection goes back.
            ListEvent::Confirm(ix) => {
                if let Some((value, false)) = rows.get(ix.row) {
                    if current != Some(ix.row) {
                        selected(tree, id, Some(value));
                    }
                    confirmed(tree, id, value);
                }
            }
            ListEvent::Cancel if current.is_some() => {
                selected(tree, id, None);
            }
            ListEvent::Cancel => return,
        }
        // Synced or not, re-sync: a controlled List goes back to the client's row.
        cx.notify();
    });
    if let Some(row) = moved {
        state.update(cx, |state, cx| {
            state.set_selected_index(row.map(IndexPath::new), window, cx);
            state.scroll_to_selected_item(window, cx);
        });
    }
}

/// Props: `placeholder` (the search field's); the rest is read by the delegate and `sync`.
pub fn build(node: &Node, slot: &ListSlot) -> List<Delegate> {
    let list = List::new(&slot.state);
    match prop_str(node, "placeholder") {
        Some(placeholder) => list.search_placeholder(placeholder.to_owned()),
        None => list,
    }
}

// User actions, shared with `simulate`.

/// A row selected with a click or the arrow keys (`None`: Escape cleared it).
pub fn selected(tree: &mut Tree, id: NodeId, value: Option<&str>) -> bool {
    chosen(tree, id, "value", value.map_or(Value::Null, Value::from))
}

pub fn confirmed(tree: &Tree, id: NodeId, value: &str) {
    emit(tree, id, "confirm", Some(value.into()), None);
}

/// What the user typed in the search field (trimmed); the client sends the rows that match.
pub fn searched(tree: &Tree, id: NodeId, query: &str) {
    emit(tree, id, "query", Some(query.into()), None);
}

pub fn more(tree: &Tree, id: NodeId) {
    emit(tree, id, "loadMore", None, None);
}

/// The row of a List with this `value`, and whether it is disabled (for `simulate`).
pub fn find(tree: &Tree, id: NodeId, value: &str) -> Option<bool> {
    items(tree, id)
        .find(|(_, item)| item_value(item) == value)
        .map(|(_, item)| prop_bool(item, "disabled"))
}

#[cfg(test)]
mod tests {
    use super::enabled_row;

    #[test]
    fn the_keys_pass_over_disabled_rows() {
        let disabled = [false, true, true, false];
        // Down from 0 lands on 1: on to 3. Up from 3 lands on 2: back to 0.
        assert_eq!(enabled_row(&disabled, 1, Some(0)), Some(3));
        assert_eq!(enabled_row(&disabled, 2, Some(3)), Some(0));
        assert_eq!(enabled_row(&disabled, 0, None), Some(0));
        // Nothing enabled past it: none (the caller keeps the current row).
        assert_eq!(enabled_row(&[false, true], 1, Some(0)), None);
    }
}
