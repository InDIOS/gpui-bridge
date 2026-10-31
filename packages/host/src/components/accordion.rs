//! `Accordion` (PROTOCOL.md section 17): a list of collapsible sections.
//!
//! The sections are `AccordionItem` children, whose children are the content, or else the
//! `items` prop (`{id, title, content?}`), whose content is text.

use std::sync::Once;

use gpui_kit::component::accordion::Accordion;
use gpui_kit::*;
use serde_json::Value;

use super::{emit, prop_bool, prop_str};
use crate::Session;
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree};

/// An `items` entry: `(id, title, content, open, disabled)`.
type Item = (String, String, String, bool, bool);

/// The `items` of an Accordion. A missing `content` is an empty body.
pub fn items(node: &Node) -> Vec<Item> {
    let text = |item: &Value, key: &str| {
        item.get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    node.props
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|item| {
            (
                text(item, "id"),
                text(item, "title"),
                text(item, "content"),
                item.get("open") == Some(&Value::Bool(true)),
                item.get("disabled") == Some(&Value::Bool(true)),
            )
        })
        .collect()
}

static WARN_ITEMS: Once = Once::new();

/// The sections of an Accordion, each with the `AccordionItem` it comes from: its
/// `AccordionItem` children if it has any (their `content` is empty: it is their children),
/// else `items`.
pub fn sections(tree: &Tree, node: &Node) -> Vec<(Item, Option<NodeId>)> {
    let parts: Vec<(Item, Option<NodeId>)> = node
        .children
        .iter()
        .filter_map(|&child| {
            let part = tree
                .node(child)
                .filter(|part| part.node_type == NodeType::AccordionItem)?;
            let text = |key| prop_str(part, key).unwrap_or("").to_owned();
            let item = (
                text("id"),
                text("title"),
                String::new(),
                prop_bool(part, "open"),
                prop_bool(part, "disabled"),
            );
            Some((item, Some(child)))
        })
        .collect();
    if parts.is_empty() {
        return items(node).into_iter().map(|item| (item, None)).collect();
    }
    if node
        .props
        .get("items")
        .is_some_and(|items| !items.is_null())
    {
        WARN_ITEMS.call_once(|| {
            log::warn!("an Accordion with AccordionItem children ignores its 'items'")
        });
    }
    parts
}

/// Checks an `items` list: objects with `id` and `title` strings, an optional `content` string
/// and optional `open` and `disabled` booleans.
pub fn check_items(value: &Value) -> Result<(), String> {
    let items = value.as_array().ok_or("a list of items")?;
    for (ix, item) in items.iter().enumerate() {
        let ok = item.get("id").is_some_and(Value::is_string)
            && item.get("title").is_some_and(Value::is_string)
            && item
                .get("content")
                .is_none_or(|content| content.is_string() || content.is_null())
            && item
                .get("open")
                .is_none_or(|open| open.is_boolean() || open.is_null())
            && item
                .get("disabled")
                .is_none_or(|disabled| disabled.is_boolean() || disabled.is_null());
        if !ok {
            return Err(format!(
                "item {ix} needs 'id' and 'title' strings (and an optional 'content', 'open' and 'disabled')"
            ));
        }
    }
    Ok(())
}

/// Props: `items`, `multiple`, `bordered` (default true), `disabled`, `controlled`.
/// Events: `change` with the id of the section the user opened or closed.
/// `sections` are the `sections` of the node, each with its title and content already built.
pub fn build(
    id: NodeId,
    node: &Node,
    session: &Entity<Session>,
    sections: Vec<(Item, AnyElement, AnyElement)>,
) -> Accordion {
    let ids: Vec<String> = sections.iter().map(|((id, ..), ..)| id.clone()).collect();
    let open: Vec<usize> = sections
        .iter()
        .enumerate()
        .filter(|(_, (item, ..))| item.3)
        .map(|(ix, _)| ix)
        .collect();
    let controlled = prop_bool(node, "controlled");
    let session = session.clone();
    let mut accordion = Accordion::new(("node", id))
        .multiple(prop_bool(node, "multiple"))
        .bordered(node.props.get("bordered") != Some(&Value::Bool(false)))
        .disabled(prop_bool(node, "disabled"));
    for ((_, _, _, is_open, disabled), title, content) in sections {
        accordion = accordion.item(move |item| {
            item.title(title)
                .open(is_open)
                .disabled(disabled)
                .child(content)
        });
    }
    accordion.on_toggle_click(move |now, _, cx| {
        // The toggled section is the one that differs from the current state.
        let Some((ix, toggled)) = ids
            .iter()
            .enumerate()
            .find(|(ix, _)| open.contains(ix) != now.contains(ix))
        else {
            return;
        };
        let (toggled, is_open) = (toggled.clone(), now.contains(&ix));
        session.update(cx, |session, cx| {
            // Synced: the host keeps the new state; controlled: only reports it.
            if !controlled {
                set_open(&mut session.tree, id, ix, is_open);
            }
            emit(
                &session.tree,
                id,
                "change",
                Some(Value::String(toggled)),
                None,
            );
            cx.notify();
        });
    })
}

/// Rewrites the `open` of one section: its `AccordionItem`'s prop, or its entry in `items`.
pub fn set_open(tree: &mut Tree, id: NodeId, ix: usize, open: bool) {
    let part = tree
        .node(id)
        .and_then(|node| sections(tree, node).get(ix).map(|section| section.1));
    if let Some(Some(part)) = part {
        tree.set_host_prop(part, "open", Value::Bool(open));
        return;
    }
    let Some(items) = tree
        .node(id)
        .and_then(|node| node.props.get("items"))
        .cloned()
    else {
        return;
    };
    let Some(items) = items.as_array().and_then(|items| {
        let mut items = items.clone();
        let item = items.get_mut(ix)?;
        let object = item.as_object_mut()?;
        object.insert("open".into(), Value::Bool(open));
        Some(Value::Array(items))
    }) else {
        return;
    };
    tree.set_host_prop(id, "items", items);
}

/// The user opened or closed a section (`simulate`): the state follows unless the node is
/// `controlled`, and the `change` event is reported either way.
pub fn toggled(tree: &mut Tree, id: NodeId, item: &str) -> Result<(), String> {
    let Some(node) = tree.node(id) else {
        return Ok(());
    };
    let list: Vec<Item> = sections(tree, node)
        .into_iter()
        .map(|(item, _)| item)
        .collect();
    let (controlled, all_disabled) = (prop_bool(node, "controlled"), prop_bool(node, "disabled"));
    let Some(ix) = list.iter().position(|entry| entry.0 == item) else {
        return Err(format!("Accordion {id} has no item '{item}'"));
    };
    let (.., open, disabled) = list[ix].clone();
    if all_disabled || disabled {
        return Ok(());
    }
    if !controlled {
        set_open(tree, id, ix, !open);
    }
    emit(
        tree,
        id,
        "change",
        Some(Value::String(item.to_owned())),
        None,
    );
    Ok(())
}
