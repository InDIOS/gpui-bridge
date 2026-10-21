//! `Sidebar` (PROTOCOL.md section 19): the sections and their entries are data in props; `value`
//! is the active entry's id. A click reports the entry with `change`, and synced mode keeps it in
//! `value`.

use std::collections::HashSet;

use gpui_kit::component::sidebar::{
    Sidebar, SidebarCollapsible, SidebarGroup, SidebarItem, SidebarMenu, SidebarMenuItem,
};
use gpui_kit::component::{Collapsible, Icon, Side};
use gpui_kit::*;
use serde_json::Value;

use super::{icon_exists, icon_path, value_chosen};
use crate::Session;
use crate::protocol::NodeId;
use crate::tree::{Node, Tree as NodeTree};

/// One entry of a section: `{id, label, icon?, disabled?, open?, children?}`, `children` a
/// submenu of entries.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub label: String,
    pub icon: Option<String>,
    pub disabled: bool,
    /// The submenu starts open (`default_open`); the user changes it from there on.
    pub open: bool,
    pub children: Vec<Entry>,
}

/// A heading and its entries; a section without `label` is a menu with no heading.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub label: Option<String>,
    pub entries: Vec<Entry>,
}

fn parse_entry(value: &Value, seen: &mut HashSet<String>) -> Result<Entry, String> {
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
    let icon = match value.get("icon") {
        None | Some(Value::Null) => None,
        Some(icon) => {
            let name = icon.as_str().ok_or("'icon' must be a string")?;
            if !icon_exists(name) {
                return Err(format!("'{name}' is not a bundled icon"));
            }
            Some(name.to_owned())
        }
    };
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
                    parse_entry(child, seen).map_err(|err| format!("child {ix} ({err})"))
                })
                .collect::<Result<Vec<_>, String>>()?
        }
    };
    Ok(Entry {
        id,
        label,
        icon,
        disabled: flag("disabled")?,
        open: flag("open")?,
        children,
    })
}

/// Checks `items`: a list of `{label?, items}`, each item an entry (see [`Entry`]), recursively.
pub fn check_items(value: &Value) -> Result<(), String> {
    let sections = value.as_array().ok_or("a list of sidebar sections")?;
    let mut seen = HashSet::new();
    for (ix, section) in sections.iter().enumerate() {
        let label = match section.get("label") {
            None | Some(Value::Null) => None,
            Some(label) => Some(
                label
                    .as_str()
                    .ok_or_else(|| format!("section {ix} ('label' must be a string)"))?
                    .to_owned(),
            ),
        };
        let entries = section
            .get("items")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("section {ix} needs a list 'items'"))?;
        for (entry_ix, entry) in entries.iter().enumerate() {
            parse_entry(entry, &mut seen)
                .map_err(|err| format!("section {ix} ({label:?}), entry {entry_ix} ({err})"))?;
        }
    }
    Ok(())
}

/// The sections a validated `items` prop holds.
fn parse(value: &Value) -> Vec<Section> {
    fn text(value: &Value, key: &str) -> String {
        value
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    }
    fn flag(value: &Value, key: &str) -> bool {
        value.get(key).and_then(Value::as_bool).unwrap_or(false)
    }
    fn entry(value: &Value) -> Entry {
        Entry {
            id: text(value, "id"),
            label: text(value, "label"),
            icon: value.get("icon").and_then(Value::as_str).map(str::to_owned),
            disabled: flag(value, "disabled"),
            open: flag(value, "open"),
            children: value
                .get("children")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(entry)
                .collect(),
        }
    }
    value
        .as_array()
        .into_iter()
        .flatten()
        .map(|section| Section {
            label: section
                .get("label")
                .and_then(Value::as_str)
                .map(str::to_owned),
            entries: section
                .get("items")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(entry)
                .collect(),
        })
        .collect()
}

/// The sections of `node`; props were validated when they were set.
pub fn sections(node: &Node) -> Vec<Section> {
    parse(node.props.get("items").unwrap_or(&Value::Null))
}

/// The entry of one section, by id.
pub fn find_entry<'a>(sections: &'a [Section], id: &str) -> Option<&'a Entry> {
    fn find<'a>(entries: &'a [Entry], id: &str) -> Option<&'a Entry> {
        for entry in entries {
            if entry.id == id {
                return Some(entry);
            }
            if let Some(found) = find(&entry.children, id) {
                return Some(found);
            }
        }
        None
    }
    sections
        .iter()
        .find_map(|section| find(&section.entries, id))
}

/// A user click on an entry: reports the chosen id (`change`) and keeps it when synced.
/// Returns whether the host changed the active id.
pub fn entry_clicked(tree: &mut NodeTree, id: NodeId, entry: &str) -> bool {
    let current = tree
        .node(id)
        .and_then(|node| node.props.get("value"))
        .and_then(Value::as_str);
    if current == Some(entry) {
        return false;
    }
    value_chosen(tree, id, Value::String(entry.to_owned()))
}

/// A section's content: a `SidebarGroup` when it has a label, a bare menu when it has none. Both
/// are `SidebarItem`s, which is what the sidebar takes in its content list.
#[derive(Clone)]
enum Item {
    Group(SidebarGroup<SidebarMenu>),
    /// Boxed: a menu carries every item of its section, and the group is two pointers.
    Menu(Box<SidebarMenu>),
}

impl Collapsible for Item {
    fn is_collapsed(&self) -> bool {
        match self {
            Self::Group(group) => group.is_collapsed(),
            Self::Menu(menu) => menu.is_collapsed(),
        }
    }

    fn collapsed(self, collapsed: bool) -> Self {
        match self {
            Self::Group(group) => Self::Group(group.collapsed(collapsed)),
            Self::Menu(menu) => Self::Menu(Box::new(menu.collapsed(collapsed))),
        }
    }
}

impl SidebarItem for Item {
    fn render(
        self,
        id: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        match self {
            Self::Group(group) => group.render(id, window, cx).into_any_element(),
            Self::Menu(menu) => (*menu).render(id, window, cx).into_any_element(),
        }
    }
}

/// One entry, with its submenu: a parent opens or closes it on click and still reports the
/// choice, like any other entry.
fn entry_element(
    id: NodeId,
    session: &Entity<Session>,
    spec: &Entry,
    active: Option<&str>,
) -> SidebarMenuItem {
    let mut item = SidebarMenuItem::new(SharedString::from(spec.label.clone()))
        .active(active == Some(spec.id.as_str()))
        .disable(spec.disabled)
        .default_open(spec.open);
    if let Some(icon) = &spec.icon {
        item = item.icon(Icon::default().path(icon_path(icon)));
    }
    if !spec.children.is_empty() {
        item = item.click_to_toggle(true).children(
            spec.children
                .iter()
                .map(|child| entry_element(id, session, child, active)),
        );
    }
    let session = session.clone();
    let chosen = spec.id.clone();
    item.on_click(move |_, _, cx| {
        session.update(cx, |session, cx| {
            if entry_clicked(&mut session.tree, id, &chosen) {
                cx.notify();
            }
        });
    })
}

fn menu(
    id: NodeId,
    session: &Entity<Session>,
    section: &Section,
    active: Option<&str>,
) -> SidebarMenu {
    SidebarMenu::new().children(
        section
            .entries
            .iter()
            .map(|spec| entry_element(id, session, spec, active)),
    )
}

/// Props: `items` (sections of entries), `value` (the active entry's id), `collapsed`,
/// `collapsible` (`icon`, `offcanvas` or `none`), `side` (`left` or `right`) and `controlled`.
/// Its content is `items`: the node takes no children.
pub fn build(id: NodeId, node: &Node, session: &Entity<Session>) -> AnyElement {
    let active = node.props.get("value").and_then(Value::as_str);
    let content: Vec<Item> = sections(node)
        .iter()
        .map(|section| {
            let menu = menu(id, session, section, active);
            match &section.label {
                Some(label) => {
                    Item::Group(SidebarGroup::new(SharedString::from(label.clone())).child(menu))
                }
                None => Item::Menu(Box::new(menu)),
            }
        })
        .collect();
    let flag = |key| {
        node.props
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    let mut sidebar = Sidebar::new(format!("sidebar-{id}"))
        .children(content)
        .collapsed(flag("collapsed"))
        .collapsible(
            match node.props.get("collapsible").and_then(Value::as_str) {
                Some("offcanvas") => SidebarCollapsible::Offcanvas,
                Some("none") => SidebarCollapsible::None,
                _ => SidebarCollapsible::Icon,
            },
        );
    if node.props.get("side").and_then(Value::as_str) == Some("right") {
        sidebar = sidebar.side(Side::Right);
    }
    sidebar.into_any_element()
}

#[cfg(test)]
mod tests {
    // Not `super::*`: gpui's own `test` attribute would shadow the standard one.
    use super::{check_items, find_entry, sections};
    use crate::schema::NodeType;
    use crate::tree::Node;
    use serde_json::{Value, json};

    fn node(props: Value) -> Node {
        Node {
            node_type: NodeType::Sidebar,
            props: props.as_object().cloned().unwrap_or_default(),
            events: Default::default(),
            children: vec![],
            parent: None,
        }
    }

    /// A node whose `items` is the list above.
    fn sidebar() -> Node {
        node(json!({ "items": items() }))
    }

    fn items() -> Value {
        json!([
            {"label": "General", "items": [
                {"id": "home", "label": "Home", "icon": "layout-dashboard"},
                {"id": "inbox", "label": "Inbox", "icon": "inbox"},
                {"id": "soon", "label": "Soon", "disabled": true}
            ]},
            {"items": [
                {"id": "settings", "label": "Settings", "open": true,
                 "children": [{"id": "theme", "label": "Theme"}]}
            ]}
        ])
    }

    #[test]
    fn items_are_checked_section_by_section() {
        assert!(check_items(&items()).is_ok());
        // A section needs `items`; an entry needs `id` and `label`; the ids are unique.
        assert!(check_items(&json!([{"label": "General"}])).is_err());
        assert!(check_items(&json!([{"items": [{"id": "a"}]}])).is_err());
        assert!(check_items(&json!([{"items": [{"label": "A"}]}])).is_err());
        assert!(
            check_items(&json!([{"items": [{"id": "a", "label": "A"}]},
                        {"items": [{"id": "a", "label": "B"}]}]))
            .is_err()
        );
        assert!(
            check_items(&json!([{"items": [{"id": "a", "label": "A", "icon": "nope"}]}])).is_err()
        );
        assert!(
            check_items(&json!([{"items": [{"id": "a", "label": "A", "open": "yes"}]}])).is_err()
        );
        assert!(
            check_items(&json!([{"items": [{"id": "a", "label": "A",
                                            "children": [{"id": "b"}]}]}]))
            .is_err()
        );
    }

    #[test]
    fn sections_keep_their_entries_and_submenus() {
        let sections = sections(&sidebar());
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].label.as_deref(), Some("General"));
        assert_eq!(sections[1].label, None);
        assert_eq!(sections[0].entries.len(), 3);
        assert!(sections[0].entries[2].disabled);
        assert!(sections[1].entries[0].open);
        assert_eq!(sections[1].entries[0].children[0].label, "Theme");
        assert_eq!(
            find_entry(&sections, "home").map(|entry| entry.label.as_str()),
            Some("Home")
        );
        assert_eq!(
            find_entry(&sections, "theme").map(|entry| entry.label.as_str()),
            Some("Theme")
        );
        assert!(find_entry(&sections, "missing").is_none());
    }
}
