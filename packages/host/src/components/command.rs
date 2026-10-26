//! `Command` (PROTOCOL.md section 21): a command palette. The sections and their entries are data
//! in props, like a Sidebar's; the component filters them by the query and reports the confirmed
//! entry's id with `confirm`. One retained `CommandState` per node keeps the query and highlight.

use std::collections::{HashMap, HashSet};

use gpui_kit::component::command::{Command, CommandGroup, CommandItem, CommandState};
use gpui_kit::component::{Disableable as _, Icon, IndexPath};
use gpui_kit::*;
use serde_json::Value;

use super::{emit, icon_exists, icon_path, prop_str};
use crate::Session;
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree};

/// One entry of a section: `{id, label, icon?, keywords?, checked?, disabled?}`.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub label: String,
    pub icon: Option<String>,
    pub keywords: Vec<String>,
    pub checked: bool,
    pub disabled: bool,
}

/// A heading and its entries; a section without `label` has no heading.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub label: Option<String>,
    pub entries: Vec<Entry>,
}

fn check_entry(value: &Value, seen: &mut HashSet<String>) -> Result<(), String> {
    let text = |key| {
        value
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("'{key}' must be a string"))
    };
    let id = text("id")?;
    text("label")?;
    if !seen.insert(id.to_owned()) {
        return Err(format!("the id '{id}' is repeated"));
    }
    match value.get("icon") {
        None | Some(Value::Null) => {}
        Some(icon) => {
            let name = icon.as_str().ok_or("'icon' must be a string")?;
            if !icon_exists(name) {
                return Err(format!("'{name}' is not a bundled icon"));
            }
        }
    }
    match value.get("keywords") {
        None | Some(Value::Null) => {}
        Some(keywords) => {
            let all_text = keywords
                .as_array()
                .is_some_and(|keywords| keywords.iter().all(Value::is_string));
            if !all_text {
                return Err("'keywords' must be a list of strings".into());
            }
        }
    }
    for key in ["checked", "disabled"] {
        if value.get(key).is_some_and(|flag| !flag.is_boolean()) {
            return Err(format!("'{key}' must be a boolean"));
        }
    }
    Ok(())
}

/// Checks `items`: a list of `{label?, items}`, each item an entry (see [`Entry`]), ids unique.
pub fn check_items(value: &Value) -> Result<(), String> {
    let sections = value.as_array().ok_or("a list of command sections")?;
    let mut seen = HashSet::new();
    for (ix, section) in sections.iter().enumerate() {
        if section
            .get("label")
            .is_some_and(|label| !label.is_string() && !label.is_null())
        {
            return Err(format!("section {ix} ('label' must be a string)"));
        }
        let entries = section
            .get("items")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("section {ix} needs a list 'items'"))?;
        for (entry_ix, entry) in entries.iter().enumerate() {
            check_entry(entry, &mut seen)
                .map_err(|err| format!("section {ix}, entry {entry_ix} ({err})"))?;
        }
    }
    Ok(())
}

/// The sections of `node`; props were validated when they were set.
pub fn sections(node: &Node) -> Vec<Section> {
    let text = |value: &Value, key| {
        value
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let flag = |value: &Value, key| value.get(key).and_then(Value::as_bool).unwrap_or(false);
    let entry = |value: &Value| Entry {
        id: text(value, "id"),
        label: text(value, "label"),
        icon: value.get("icon").and_then(Value::as_str).map(str::to_owned),
        keywords: value
            .get("keywords")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        checked: flag(value, "checked"),
        disabled: flag(value, "disabled"),
    };
    node.props
        .get("items")
        .and_then(Value::as_array)
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

pub fn find_entry<'a>(sections: &'a [Section], id: &str) -> Option<&'a Entry> {
    sections
        .iter()
        .flat_map(|section| &section.entries)
        .find(|entry| entry.id == id)
}

/// The user confirmed an entry (Enter or a click): reports its id. A palette holds no value, so
/// nothing is kept in the host.
pub fn confirmed(tree: &Tree, id: NodeId, entry: &str) {
    emit(
        tree,
        id,
        "confirm",
        Some(Value::String(entry.to_owned())),
        None,
    );
}

/// The query, highlight and focus of one palette; the entries are passed on every render.
pub struct CommandSlot {
    state: Entity<CommandState>,
}

impl CommandSlot {
    /// Focuses the search field, so a palette opened in a Dialog can be typed into at once.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        let handle = self.state.read(cx).focus_handle(cx);
        handle.focus(window, cx);
    }
}

/// The Commands among `members`.
pub fn snapshot(tree: &Tree, members: &HashSet<NodeId>) -> HashSet<NodeId> {
    members
        .iter()
        .copied()
        .filter(|&id| {
            tree.node(id)
                .is_some_and(|node| node.node_type == NodeType::Command)
        })
        .collect()
}

/// Creates a state for each Command in `commands` and drops the removed ones.
pub fn sync<V: 'static>(
    slots: &mut HashMap<NodeId, CommandSlot>,
    commands: HashSet<NodeId>,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    slots.retain(|id, _| commands.contains(id));
    for id in commands {
        slots.entry(id).or_insert_with(|| CommandSlot {
            state: cx.new(|cx| CommandState::new(window, cx)),
        });
    }
}

/// Props: `items` (sections of entries) and `placeholder`. Each section is a group, so the
/// palette's index path is (section, entry) and maps back to the entry's id.
pub fn build(id: NodeId, node: &Node, slot: &CommandSlot, session: &Entity<Session>) -> Command {
    let sections = sections(node);
    let mut command = Command::new(&slot.state);
    for section in &sections {
        let mut group = CommandGroup::new();
        if let Some(label) = &section.label {
            group = group.label(label.clone());
        }
        command = command.group(group.items(section.entries.iter().map(|entry| {
            let mut item = CommandItem::new()
                .label(entry.label.clone())
                .keywords(entry.keywords.clone())
                .checked(entry.checked)
                .disabled(entry.disabled);
            if let Some(icon) = &entry.icon {
                item = item.icon(Icon::default().path(icon_path(icon)));
            }
            item
        })));
    }
    if let Some(placeholder) = prop_str(node, "placeholder") {
        command = command.placeholder(placeholder.to_owned());
    }
    let session = session.clone();
    command.on_confirm(move |path: IndexPath, _, cx| {
        let Some(entry) = sections
            .get(path.section)
            .and_then(|section| section.entries.get(path.row))
        else {
            return;
        };
        confirmed(&session.read(cx).tree, id, &entry.id);
    })
}

#[cfg(test)]
mod tests {
    // Not `super::*`: gpui's own `test` attribute would shadow the standard one.
    use super::{check_items, find_entry, sections};
    use crate::schema::NodeType;
    use crate::tree::Node;
    use serde_json::{Value, json};

    fn items() -> Value {
        json!([
            {"label": "File", "items": [
                {"id": "open", "label": "Open", "icon": "folder", "keywords": ["load"]},
                {"id": "save", "label": "Save", "checked": true},
                {"id": "soon", "label": "Soon", "disabled": true}
            ]},
            {"items": [{"id": "quit", "label": "Quit"}]}
        ])
    }

    #[test]
    fn items_are_checked_section_by_section() {
        assert!(check_items(&items()).is_ok());
        assert!(check_items(&json!([{"label": "File"}])).is_err());
        assert!(check_items(&json!([{"label": 3, "items": []}])).is_err());
        assert!(check_items(&json!([{"items": [{"id": "a"}]}])).is_err());
        assert!(
            check_items(&json!([{"items": [{"id": "a", "label": "A"}]},
                        {"items": [{"id": "a", "label": "B"}]}]))
            .is_err()
        );
        assert!(
            check_items(&json!([{"items": [{"id": "a", "label": "A", "icon": "nope"}]}])).is_err()
        );
        assert!(
            check_items(&json!([{"items": [{"id": "a", "label": "A", "keywords": [1]}]}])).is_err()
        );
        assert!(
            check_items(&json!([{"items": [{"id": "a", "label": "A", "checked": "yes"}]}]))
                .is_err()
        );
    }

    #[test]
    fn sections_keep_their_entries() {
        let node = Node {
            node_type: NodeType::Command,
            props: json!({ "items": items() }).as_object().cloned().unwrap(),
            events: Default::default(),
            children: vec![],
            parent: None,
        };
        let sections = sections(&node);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].label.as_deref(), Some("File"));
        assert_eq!(sections[1].label, None);
        assert_eq!(sections[0].entries[0].keywords, ["load"]);
        assert!(sections[0].entries[1].checked);
        assert!(sections[0].entries[2].disabled);
        assert_eq!(
            find_entry(&sections, "quit").map(|e| e.label.as_str()),
            Some("Quit")
        );
        assert!(find_entry(&sections, "missing").is_none());
    }
}
