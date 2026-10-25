//! `Settings` (PROTOCOL.md section 22): pages of grouped fields, all data in props. A field the
//! user changes keeps its new `value` in the host and is reported with `change` as `{key, value}`.
//! gpui-kit's Settings keeps the selected page and the search in its own keyed state: no slot.

use std::collections::HashSet;

use gpui_kit::component::Icon;
use gpui_kit::component::setting::{
    NumberFieldOptions, SettingField, SettingGroup, SettingItem, SettingPage, Settings,
};
use gpui_kit::*;
use serde_json::{Value, json};

use super::{check_options, emit_change, icon_exists, icon_path};
use crate::Session;
use crate::protocol::NodeId;
use crate::tree::{Node, Tree};

const TYPES: &[&str] = &["switch", "checkbox", "input", "dropdown", "number"];

/// Where an item sits: (page, group, item). gpui-kit keys a field's state by the same indices.
pub type Path = (usize, usize, usize);

/// Whether `value` fits a field of `item`'s type (a dropdown's must be one of its options).
pub fn fits(item: &Value, value: &Value) -> bool {
    match item.get("type").and_then(Value::as_str) {
        Some("switch" | "checkbox") => value.is_boolean(),
        Some("input") => value.is_string(),
        Some("number") => value.as_f64().is_some_and(f64::is_finite),
        Some("dropdown") => item
            .get("options")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|option| option.get("value") == Some(value)),
        _ => false,
    }
}

fn check_item(item: &Value, seen: &mut HashSet<String>) -> Result<(), String> {
    let text = |key| {
        item.get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("'{key}' must be a string"))
    };
    let key = text("key")?;
    text("label")?;
    if !seen.insert(key.to_owned()) {
        return Err(format!("the key '{key}' is repeated"));
    }
    if item
        .get("description")
        .is_some_and(|d| !d.is_string() && !d.is_null())
    {
        return Err("'description' must be a string".into());
    }
    let kind = text("type")?;
    if !TYPES.contains(&kind) {
        return Err(format!("'type' must be one of {}", TYPES.join(", ")));
    }
    if kind == "dropdown" {
        check_options(item.get("options").unwrap_or(&Value::Null))
            .map_err(|err| format!("'options' ({err})"))?;
    }
    if !item.get("value").is_some_and(|value| fits(item, value)) {
        return Err(format!("'value' does not fit a {kind} field"));
    }
    for key in ["min", "max", "step"] {
        if item
            .get(key)
            .is_some_and(|n| !n.is_null() && !n.as_f64().is_some_and(f64::is_finite))
        {
            return Err(format!("'{key}' must be a number"));
        }
    }
    if item
        .get("step")
        .and_then(Value::as_f64)
        .is_some_and(|step| step <= 0.)
    {
        return Err("'step' must be above 0".into());
    }
    // gpui-kit clamps typed numbers with `f64::clamp`, which panics when min > max.
    let bound = |key| item.get(key).and_then(Value::as_f64);
    if let (Some(min), Some(max)) = (bound("min"), bound("max"))
        && min > max
    {
        return Err("'min' must not be above 'max'".into());
    }
    if item.get("disabled").is_some_and(|d| !d.is_boolean()) {
        return Err("'disabled' must be a boolean".into());
    }
    Ok(())
}

/// Checks `pages`: `[{title, icon?, groups: [{title?, items: [item]}]}]`, keys unique.
pub fn check_pages(value: &Value) -> Result<(), String> {
    let pages = value.as_array().ok_or("a list of settings pages")?;
    let mut seen = HashSet::new();
    for (p, page) in pages.iter().enumerate() {
        if !page.get("title").is_some_and(Value::is_string) {
            return Err(format!("page {p} needs a string 'title'"));
        }
        match page.get("icon") {
            None | Some(Value::Null) => {}
            Some(icon) if icon.as_str().is_some_and(icon_exists) => {}
            Some(icon) => return Err(format!("page {p} ('{icon}' is not a bundled icon)")),
        }
        let groups = page
            .get("groups")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("page {p} needs a list 'groups'"))?;
        for (g, group) in groups.iter().enumerate() {
            if group
                .get("title")
                .is_some_and(|t| !t.is_string() && !t.is_null())
            {
                return Err(format!("page {p}, group {g} ('title' must be a string)"));
            }
            let items = group
                .get("items")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("page {p}, group {g} needs a list 'items'"))?;
            for (i, item) in items.iter().enumerate() {
                check_item(item, &mut seen)
                    .map_err(|err| format!("page {p}, group {g}, item {i} ({err})"))?;
            }
        }
    }
    Ok(())
}

fn list<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

fn pages(node: &Node) -> impl Iterator<Item = &Value> {
    node.props
        .get("pages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

/// The path and spec of the item with `key`.
pub fn find<'a>(node: &'a Node, key: &str) -> Option<(Path, &'a Value)> {
    pages(node).enumerate().find_map(|(p, page)| {
        list(page, "groups").enumerate().find_map(|(g, group)| {
            list(group, "items")
                .enumerate()
                .find(|(_, item)| item.get("key").and_then(Value::as_str) == Some(key))
                .map(|(i, item)| ((p, g, i), item))
        })
    })
}

/// The user set the item at `path` to `value`: the host keeps it and reports `{key, value}`.
/// Returns false when nothing changed (the fields also call this to sync a value set by props).
pub fn changed(tree: &mut Tree, id: NodeId, (p, g, i): Path, value: Value) -> bool {
    let Some(mut pages) = tree
        .node(id)
        .and_then(|node| node.props.get("pages"))
        .cloned()
    else {
        return false;
    };
    let Some(item) = pages.pointer_mut(&format!("/{p}/groups/{g}/items/{i}")) else {
        return false;
    };
    if item.get("value") == Some(&value) {
        return false;
    }
    item["value"] = value.clone();
    let key = item["key"].clone();
    tree.set_host_prop(id, "pages", pages);
    emit_change(tree, id, Some(json!({ "key": key, "value": value })), None);
    true
}

/// One item as a gpui-kit field. The setter looks the item up by `path` when it runs: the string
/// and number fields keep the first setter they get for as long as their keyed state lives.
fn item(id: NodeId, path: Path, spec: &Value, session: &Entity<Session>) -> SettingItem {
    let set = {
        let session = session.clone();
        move |value: Value, cx: &mut App| {
            session.update(cx, |session, cx| {
                if changed(&mut session.tree, id, path, value) {
                    cx.notify();
                }
            });
        }
    };
    let text = |key| {
        spec.get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let number = |key| spec.get(key).and_then(Value::as_f64);
    let value = &spec["value"];
    let label = text("label");
    let item = match spec.get("type").and_then(Value::as_str) {
        Some(kind @ ("switch" | "checkbox")) => {
            let on = value.as_bool().unwrap_or(false);
            let set = move |on, cx: &mut App| set(Value::Bool(on), cx);
            if kind == "switch" {
                SettingItem::new(label, SettingField::switch(move |_| on, set))
            } else {
                SettingItem::new(label, SettingField::checkbox(move |_| on, set))
            }
        }
        Some("number") => {
            let defaults = NumberFieldOptions::default();
            let options = NumberFieldOptions {
                min: number("min").unwrap_or(defaults.min),
                max: number("max").unwrap_or(defaults.max),
                step: number("step").unwrap_or(defaults.step),
            };
            let n = value.as_f64().unwrap_or_default();
            SettingItem::new(
                label,
                SettingField::number_input(
                    options,
                    move |_| n,
                    // A whole number goes out as one (`11`, not `11.0`), like the props it came from.
                    move |n: f64, cx: &mut App| {
                        if n.fract() == 0. && n.abs() < 1e15 {
                            set(json!(n as i64), cx)
                        } else {
                            set(json!(n), cx)
                        }
                    },
                ),
            )
        }
        Some("dropdown") => {
            let text = |option: &Value, key| {
                SharedString::from(option[key].as_str().unwrap_or_default().to_owned())
            };
            let choices = list(spec, "options")
                .map(|option| (text(option, "value"), text(option, "label")))
                .collect();
            let current = SharedString::from(value.as_str().unwrap_or_default().to_owned());
            SettingItem::new(
                label,
                SettingField::dropdown(
                    choices,
                    move |_| current.clone(),
                    move |v: SharedString, cx: &mut App| set(Value::String(v.into()), cx),
                ),
            )
        }
        _ => {
            let current = SharedString::from(value.as_str().unwrap_or_default().to_owned());
            SettingItem::new(
                label,
                SettingField::input(
                    move |_| current.clone(),
                    move |v: SharedString, cx: &mut App| set(Value::String(v.into()), cx),
                ),
            )
        }
    };
    let item = item.disabled(
        spec.get("disabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    );
    match spec.get("description").and_then(Value::as_str) {
        Some(description) => item.description(description.to_owned()),
        None => item,
    }
}

/// Props: `pages`. Each page is a sidebar entry; its groups and items render on the right.
pub fn build(id: NodeId, node: &Node, session: &Entity<Session>) -> Settings {
    let pages = pages(node).enumerate().map(|(p, page)| {
        let mut out = SettingPage::new(page["title"].as_str().unwrap_or_default().to_owned());
        if let Some(icon) = page.get("icon").and_then(Value::as_str) {
            out = out.icon(Icon::default().path(icon_path(icon)));
        }
        out.groups(list(page, "groups").enumerate().map(|(g, group)| {
            let mut out = SettingGroup::new();
            if let Some(title) = group.get("title").and_then(Value::as_str) {
                out = out.title(title.to_owned());
            }
            out.items(
                list(group, "items")
                    .enumerate()
                    .map(|(i, spec)| item(id, (p, g, i), spec, session)),
            )
        }))
    });
    Settings::new(("settings", id)).pages(pages.collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    // Not `super::*`: gpui's own `test` attribute would shadow the standard one.
    use super::{check_pages, find};
    use crate::schema::NodeType;
    use crate::tree::Node;
    use serde_json::{Value, json};

    fn pages() -> Value {
        json!([
            {"title": "General", "icon": "settings", "groups": [
                {"title": "Look", "items": [
                    {"key": "dark", "label": "Dark mode", "type": "switch", "value": true},
                    {"key": "lines", "label": "Line numbers", "type": "checkbox", "value": false},
                    {"key": "font", "label": "Font", "type": "dropdown", "value": "mono",
                     "options": [{"value": "mono", "label": "Mono"}, {"value": "sans", "label": "Sans"}]}
                ]}
            ]},
            {"title": "Editor", "groups": [{"items": [
                {"key": "name", "label": "Name", "type": "input", "value": "", "description": "Shown in the title"},
                {"key": "size", "label": "Size", "type": "number", "value": 14, "min": 8, "max": 32, "step": 1, "disabled": true}
            ]}]}
        ])
    }

    fn item(extra: Value) -> Value {
        let mut item = json!({"key": "k", "label": "K", "type": "switch", "value": true});
        item.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        json!([{"title": "P", "groups": [{"items": [item]}]}])
    }

    #[test]
    fn pages_are_checked_item_by_item() {
        assert!(check_pages(&pages()).is_ok());
        assert!(check_pages(&json!([{"groups": []}])).is_err());
        assert!(check_pages(&json!([{"title": "P"}])).is_err());
        assert!(check_pages(&json!([{"title": "P", "icon": "nope", "groups": []}])).is_err());
        assert!(check_pages(&item(json!({"type": "slider"}))).is_err());
        assert!(check_pages(&item(json!({"value": "on"}))).is_err());
        assert!(check_pages(&item(json!({"type": "number", "value": "3"}))).is_err());
        assert!(check_pages(&item(json!({"type": "input", "value": 3}))).is_err());
        assert!(check_pages(&item(json!({"type": "dropdown", "value": "a"}))).is_err());
        assert!(
            check_pages(&item(json!({"type": "dropdown", "value": "b",
                "options": [{"value": "a", "label": "A"}]})))
            .is_err()
        );
        assert!(check_pages(&item(json!({"type": "number", "value": 1, "step": 0}))).is_err());
        assert!(
            check_pages(&item(
                json!({"type": "number", "value": 10, "min": 32, "max": 8})
            ))
            .is_err()
        );
        assert!(check_pages(&item(json!({"disabled": "yes"}))).is_err());
        let twice = json!([{"title": "P", "groups": [{"items": [
            {"key": "k", "label": "A", "type": "switch", "value": true},
            {"key": "k", "label": "B", "type": "switch", "value": true}
        ]}]}]);
        assert!(check_pages(&twice).is_err());
    }

    #[test]
    fn items_are_found_by_key() {
        let node = Node {
            node_type: NodeType::Settings,
            props: json!({ "pages": pages() }).as_object().cloned().unwrap(),
            events: Default::default(),
            children: vec![],
            parent: None,
        };
        assert_eq!(find(&node, "font").map(|(path, _)| path), Some((0, 0, 2)));
        assert_eq!(find(&node, "size").map(|(path, _)| path), Some((1, 0, 1)));
        assert!(find(&node, "missing").is_none());
    }
}
