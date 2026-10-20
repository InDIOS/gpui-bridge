//! `Accordion` (PROTOCOL.md section 17): a list of collapsible sections.
//!
//! The tree model has no per-item nodes, so the sections are the `items` prop
//! (`{id, title, content?}`), and their content is text. A client that needs rich content
//! inside a section uses `Collapsible` with its own children.

use gpui_kit::component::accordion::Accordion;
use gpui_kit::*;
use serde_json::Value;

use super::{emit, prop_bool};
use crate::Session;
use crate::protocol::NodeId;
use crate::tree::Node;

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
pub fn build(id: NodeId, node: &Node, session: &Entity<Session>) -> Accordion {
    let items = items(node);
    let ids: Vec<String> = items.iter().map(|(id, ..)| id.clone()).collect();
    let open: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.3)
        .map(|(ix, _)| ix)
        .collect();
    let controlled = prop_bool(node, "controlled");
    let session = session.clone();
    let mut accordion = Accordion::new(("node", id))
        .multiple(prop_bool(node, "multiple"))
        .bordered(node.props.get("bordered") != Some(&Value::Bool(false)))
        .disabled(prop_bool(node, "disabled"));
    for (_, title, content, is_open, disabled) in items {
        accordion = accordion.item(move |item| {
            item.title(SharedString::from(title))
                .open(is_open)
                .disabled(disabled)
                .child(SharedString::from(content))
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
            // Synced: the host keeps the new state in `items`; controlled: only reports it.
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

/// Rewrites the `open` of one entry of the `items` prop.
pub fn set_open(tree: &mut crate::tree::Tree, id: NodeId, ix: usize, open: bool) {
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
pub fn toggled(tree: &mut crate::tree::Tree, id: NodeId, item: &str) -> Result<(), String> {
    let Some(node) = tree.node(id) else {
        return Ok(());
    };
    let list = items(node);
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
