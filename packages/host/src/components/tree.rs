//! `Tree` (PROTOCOL.md section 18): one retained `TreeState` per node. The items are data in
//! props; rebuilding them keeps the selection the client asked for and the expansion the user
//! had already done, because `TreeState::set_items` resets both.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui_kit::component::list::ListItem;
use gpui_kit::component::tree::{Tree, TreeEvent, TreeItem, TreeState};
use gpui_kit::component::{Icon, h_flex};
use gpui_kit::*;
use serde_json::Value;

use super::{icon_path, value_chosen};
use crate::Session;
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree as NodeTree};

/// One item, as comparable data: `items` is rebuilt only when this changes.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemSpec {
    pub id: String,
    pub label: String,
    pub expanded: bool,
    pub disabled: bool,
    pub children: Vec<ItemSpec>,
}

pub struct TreeSlot {
    state: Entity<TreeState>,
    /// What the state was built from.
    items: Vec<ItemSpec>,
    /// Expansion the user chose, by item id: applied again after rebuilding `items`.
    expanded: Rc<RefCell<HashMap<String, bool>>>,
    /// Set while the host itself changes the state, so `expand`/`collapse` are not reported.
    quiet: Rc<Cell<bool>>,
    _events: Subscription,
}

impl TreeSlot {
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.state.update(cx, |state, cx| state.focus(window, cx));
    }
}

/// Each Tree among `members`: its items and the selected item id.
pub struct TreeSnapshot(Vec<(NodeId, Vec<ItemSpec>, Option<String>)>);

fn parse_item(value: &Value, seen: &mut HashSet<String>) -> Result<ItemSpec, String> {
    let text = |key| {
        value
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("'{key}' must be a string"))
    };
    let id = text("id")?.to_owned();
    let label = text("label")?.to_owned();
    if !seen.insert(id.clone()) {
        return Err(format!("the id '{id}' is repeated"));
    }
    let flag = |key| {
        value
            .get(key)
            .map(|flag| {
                flag.as_bool()
                    .ok_or_else(|| format!("'{key}' must be a boolean"))
            })
            .transpose()
            .map(Option::unwrap_or_default)
    };
    let children = match value.get("children") {
        None | Some(Value::Null) => Vec::new(),
        Some(children) => {
            let children = children.as_array().ok_or("'children' must be a list")?;
            children
                .iter()
                .enumerate()
                .map(|(ix, child)| {
                    parse_item(child, seen).map_err(|err| format!("child {ix} ({err})"))
                })
                .collect::<Result<Vec<_>, String>>()?
        }
    };
    Ok(ItemSpec {
        id,
        label,
        expanded: flag("expanded")?,
        disabled: flag("disabled")?,
        children,
    })
}

/// Checks `items`: a list of `{id, label, children?, expanded?, disabled?}`, recursively.
pub fn check_items(value: &Value) -> Result<(), String> {
    let items = value.as_array().ok_or("a list of tree items")?;
    let mut seen = HashSet::new();
    for (ix, item) in items.iter().enumerate() {
        parse_item(item, &mut seen).map_err(|err| format!("item {ix} ({err})"))?;
    }
    Ok(())
}

/// The `items` prop as comparable data (empty while it is missing or invalid).
pub fn items(node: &Node) -> Vec<ItemSpec> {
    let Some(items) = node.props.get("items").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    items
        .iter()
        .filter_map(|item| parse_item(item, &mut seen).ok())
        .collect()
}

pub fn snapshot(tree: &NodeTree, members: &HashSet<NodeId>) -> TreeSnapshot {
    TreeSnapshot(
        tree.nodes()
            .filter(|(id, node)| node.node_type == NodeType::Tree && members.contains(id))
            .map(|(id, node)| {
                let value = node
                    .props
                    .get("value")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                (id, items(node), value)
            })
            .collect(),
    )
}

fn find<'a>(items: &'a [TreeItem], id: &str) -> Option<&'a TreeItem> {
    for item in items {
        if item.id == id {
            return Some(item);
        }
        if let Some(found) = find(&item.children, id) {
            return Some(found);
        }
    }
    None
}

fn native(specs: &[ItemSpec], expanded: &HashMap<String, bool>) -> Vec<TreeItem> {
    specs
        .iter()
        .map(|spec| {
            TreeItem::new(spec.id.clone(), spec.label.clone())
                .expanded(expanded.get(&spec.id).copied().unwrap_or(spec.expanded))
                .disabled(spec.disabled)
                .children(native(&spec.children, expanded))
        })
        .collect()
}

/// A flag per item id, to tell an explicit change in `items` from an unrelated rebuild.
fn flags(specs: &[ItemSpec], out: &mut HashMap<String, bool>) {
    for spec in specs {
        out.insert(spec.id.clone(), spec.expanded);
        flags(&spec.children, out);
    }
}

/// The spec of one item, by id.
pub fn find_spec<'a>(specs: &'a [ItemSpec], id: &str) -> Option<&'a ItemSpec> {
    for spec in specs {
        if spec.id == id {
            return Some(spec);
        }
        if let Some(found) = find_spec(&spec.children, id) {
            return Some(found);
        }
    }
    None
}

/// Reports a user expansion from `simulate` (`expand` or `collapse`).
pub fn expansion_reported(tree: &NodeTree, id: NodeId, item: &str, expanded: bool) {
    let event = if expanded { "expand" } else { "collapse" };
    let value = Value::String(item.to_owned());
    super::emit(tree, id, event, Some(value), None);
}

/// Creates state, drops removed ones, rebuilds only when `items` changed, and shows each node's
/// selected id (which also reverts a controlled Tree until the client confirms it).
pub fn sync<V: 'static>(
    slots: &mut HashMap<NodeId, TreeSlot>,
    snapshot: TreeSnapshot,
    session: &Entity<Session>,
    cx: &mut Context<V>,
) {
    slots.retain(|id, _| snapshot.0.iter().any(|(tree, ..)| tree == id));
    for (id, specs, value) in snapshot.0 {
        let stale = slots.get(&id).is_none_or(|slot| slot.items != specs);
        if stale {
            // User expansion survives a rebuild only for items that still exist and whose
            // `expanded` prop the client did not change.
            let mut before = HashMap::new();
            let mut after = HashMap::new();
            if let Some(slot) = slots.get(&id) {
                flags(&slot.items, &mut before);
            }
            flags(&specs, &mut after);
            let expanded = slots
                .get(&id)
                .map(|slot| slot.expanded.clone())
                .unwrap_or_default();
            expanded
                .borrow_mut()
                .retain(|item, _| after.contains_key(item) && before.get(item) == after.get(item));
            let state = match slots.get(&id) {
                Some(slot) => slot.state.clone(),
                None => {
                    let state =
                        cx.new(|cx| TreeState::new(cx).items(native(&specs, &expanded.borrow())));
                    let session = session.clone();
                    let quiet = Rc::new(Cell::new(false));
                    let slot_quiet = quiet.clone();
                    let slot_expanded = expanded.clone();
                    let events = cx.subscribe(&state, move |_, _, event: &TreeEvent, cx| {
                        let (event, item) = match event {
                            TreeEvent::Expanded(item) => ("expand", item),
                            TreeEvent::Collapsed(item) => ("collapse", item),
                        };
                        slot_expanded
                            .borrow_mut()
                            .insert(item.to_string(), event == "expand");
                        if slot_quiet.get() {
                            return;
                        }
                        let value = Value::String(item.to_string());
                        session.update(cx, |session, _| {
                            super::emit(&session.tree, id, event, Some(value), None);
                        });
                    });
                    slots.insert(
                        id,
                        TreeSlot {
                            state: state.clone(),
                            items: Vec::new(),
                            expanded: expanded.clone(),
                            quiet,
                            _events: events,
                        },
                    );
                    state
                }
            };
            let built = native(&specs, &expanded.borrow());
            let selected = value.as_deref().and_then(|id| find(&built, id)).cloned();
            let quiet = slots[&id].quiet.clone();
            quiet.set(true);
            state.update(cx, |state, cx| {
                state.set_items(built, cx);
                state.set_selected_item(selected.as_ref(), cx);
            });
            quiet.set(false);
            if let Some(slot) = slots.get_mut(&id) {
                slot.items = specs;
            }
        } else {
            let slot = &slots[&id];
            let shown = slot
                .state
                .read(cx)
                .selected_item()
                .map(|item| item.id.to_string());
            if shown != value {
                let built = native(&slot.items, &slot.expanded.borrow());
                let selected = value.as_deref().and_then(|id| find(&built, id)).cloned();
                slot.quiet.set(true);
                slot.state.update(cx, |state, cx| {
                    state.set_selected_item(selected.as_ref(), cx)
                });
                slot.quiet.set(false);
            }
        }
    }
}

/// A user click on an item: reports the chosen id (`change`) and keeps it when synced.
/// Returns whether the host changed the selected id.
pub fn item_clicked(tree: &mut NodeTree, id: NodeId, item: &str) -> bool {
    let current = tree
        .node(id)
        .and_then(|node| node.props.get("value"))
        .and_then(Value::as_str);
    if current == Some(item) {
        return false;
    }
    value_chosen(tree, id, Value::String(item.to_owned()))
}

/// Props: `items` (applied by `sync`), `value` (the selected item's id) and `controlled`.
/// Expansion is the component's: it is remembered while the node lives and reported with
/// `expand`/`collapse`.
pub fn build(id: NodeId, slot: &TreeSlot, session: &Entity<Session>) -> Tree {
    let click = session.clone();
    Tree::new(&slot.state, move |ix, entry, _, _, _| {
        let item = entry.item().id.to_string();
        let depth = entry.depth();
        let folder = entry.is_folder();
        // `ListItem` stacks any direct children in a block: the icon and the label go in one
        // horizontal row instead.
        let mut content = h_flex().items_center().gap_2();
        if folder {
            let icon = if entry.is_expanded() {
                "chevron-down"
            } else {
                "chevron-right"
            };
            content = content.child(Icon::default().path(icon_path(icon)));
        } else {
            content = content.child(div().w(px(16.)));
        }
        content = content.child(entry.item().label.clone());
        let row = ListItem::new(ElementId::Name(format!("tree-{id}-{ix}").into()))
            .accessibility_label(entry.item().label.clone())
            .pl(px(4.0 + depth as f32 * 16.0))
            .child(content);
        if entry.is_disabled() {
            return row;
        }
        let session = click.clone();
        let clicked = item.clone();
        row.on_click(move |_, _, cx| {
            session.update(cx, |session, cx| {
                item_clicked(&mut session.tree, id, &clicked);
                // A controlled Tree selects the item in the state too: re-sync puts the
                // client's `value` back until it confirms the change.
                cx.notify();
            });
        })
    })
}

#[cfg(test)]
mod tests {
    // Not `super::*`: gpui's own `test` attribute would shadow the standard one.
    use std::collections::HashMap;

    use super::{ItemSpec, check_items, find_spec, native};
    use serde_json::json;

    #[test]
    fn items_are_checked_recursively() {
        assert!(
            check_items(&json!([
                {"id": "src", "label": "src", "expanded": true, "children": [
                    {"id": "src/lib.rs", "label": "lib.rs", "disabled": false}
                ]},
                {"id": "README.md", "label": "README.md"}
            ]))
            .is_ok()
        );
        assert!(check_items(&json!([{"id": "a"}])).is_err());
        assert!(check_items(&json!([{"id": "a", "label": "A", "children": "none"}])).is_err());
        assert!(check_items(&json!([{"id": "a", "label": "A", "expanded": "yes"}])).is_err());
        assert!(
            check_items(&json!([{"id": "a", "label": "A"}, {"id": "a", "label": "B"}])).is_err()
        );
        assert!(
            check_items(&json!([{"id": "a", "label": "A", "children": [{"label": "B"}]}])).is_err()
        );
    }

    #[test]
    fn a_user_expansion_survives_a_rebuild() {
        let specs = vec![ItemSpec {
            id: "src".into(),
            label: "src".into(),
            expanded: false,
            disabled: false,
            children: vec![ItemSpec {
                id: "src/lib.rs".into(),
                label: "lib.rs".into(),
                expanded: false,
                disabled: false,
                children: Vec::new(),
            }],
        }];
        let mut expanded = HashMap::new();
        expanded.insert("src".to_string(), true);
        let built = native(&specs, &expanded);
        assert!(built[0].is_expanded());
        assert!(!built[0].children[0].is_expanded());
        assert_eq!(
            find_spec(&specs, "src/lib.rs").map(|spec| spec.label.as_str()),
            Some("lib.rs")
        );
        assert!(find_spec(&specs, "missing").is_none());
    }
}
