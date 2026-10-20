//! Menus (PROTOCOL.md section 10): `contextMenu` on `div` and the `MenuBar` node share one item
//! format. Choosing an item sends the node's `menu` event with the item's `id` as `value`.

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::menu::{DropdownMenu, PopupMenu, PopupMenuItem};
use gpui_kit::component::{Icon, Sizable, h_flex};
use gpui_kit::*;
use serde_json::Value;

use super::{emit, icon_path};
use crate::Session;
use crate::protocol::NodeId;
use crate::tree::Tree;

/// Checks a menu item list; the error says which item is wrong.
pub fn check_items(value: &Value) -> Result<(), String> {
    let items = value.as_array().ok_or("a list of menu items")?;
    for (ix, item) in items.iter().enumerate() {
        check_item(item).map_err(|err| format!("item {ix}: {err}"))?;
    }
    Ok(())
}

fn check_item(item: &Value) -> Result<(), String> {
    let item = item.as_object().ok_or("must be an object")?;
    if item.get("separator") == Some(&Value::Bool(true)) {
        return Ok(());
    }
    let field = |key: &str, ok: fn(&Value) -> bool, what: &str| match item.get(key) {
        Some(value) if !value.is_null() && !ok(value) => Err(format!("'{key}' must be {what}")),
        _ => Ok(()),
    };
    if !item.get("label").is_some_and(Value::is_string) {
        return Err("needs a 'label' string (or 'separator': true)".into());
    }
    field("disabled", Value::is_boolean, "a boolean")?;
    field("checked", Value::is_boolean, "a boolean")?;
    field(
        "icon",
        |v| v.as_str().is_some_and(super::icon_exists),
        "a bundled icon name",
    )?;
    field(
        "shortcut",
        |v| v.as_str().and_then(crate::keys::parse).is_some(),
        "a key such as \"ctrl-s\"",
    )?;
    match (item.get("items"), item.get("id")) {
        (Some(items), _) => check_items(items).map_err(|err| format!("'items' {err}")),
        (None, Some(Value::String(_))) => Ok(()),
        _ => Err("needs an 'id' string or a submenu 'items'".into()),
    }
}

/// Checks a `MenuBar`'s `menus`: a list of `{label, items}`.
pub fn check_menus(value: &Value) -> Result<(), String> {
    let menus = value.as_array().ok_or("a list of menus")?;
    for (ix, menu) in menus.iter().enumerate() {
        if !menu.get("label").is_some_and(Value::is_string) {
            return Err(format!("menu {ix} needs a 'label' string"));
        }
        let items = menu
            .get("items")
            .ok_or_else(|| format!("menu {ix} needs 'items'"))?;
        check_items(items).map_err(|err| format!("menu {ix} {err}"))?;
    }
    Ok(())
}

/// The user chose a menu item of node `id`; shared with `simulate`.
pub fn chosen(tree: &Tree, id: NodeId, item: &str) {
    emit(tree, id, "menu", Some(item.into()), None);
}

/// Finds item `item` in `items`, submenus included: `Some(enabled)`, or `None` if it is not there.
/// An item inside a disabled submenu is not enabled.
pub fn find_item(items: &Value, item: &str) -> Option<bool> {
    items.as_array()?.iter().find_map(|entry| {
        let enabled = !flag(entry, "disabled");
        match entry.get("items") {
            Some(sub) => find_item(sub, item).map(|found| found && enabled),
            None => (entry.get("id").and_then(Value::as_str) == Some(item)).then_some(enabled),
        }
    })
}

/// The enabled item whose `shortcut` is `pressed`, submenus included.
pub fn find_shortcut(items: &Value, pressed: &Keystroke) -> Option<String> {
    items
        .as_array()?
        .iter()
        .filter(|entry| !flag(entry, "disabled"))
        .find_map(|entry| match entry.get("items") {
            Some(sub) => find_shortcut(sub, pressed),
            None => {
                let target = entry
                    .get("shortcut")
                    .and_then(Value::as_str)
                    .and_then(crate::keys::parse)?;
                crate::keys::matches(pressed, &target)
                    .then(|| entry.get("id").and_then(Value::as_str).map(str::to_owned))?
            }
        })
}

pub fn flag(item: &Value, key: &str) -> bool {
    item.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// Adds validated `items` to `menu`; each item reports its choice as node `id`'s `menu` event.
pub fn build(
    menu: PopupMenu,
    items: &[Value],
    session: &Entity<Session>,
    id: NodeId,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    items.iter().fold(menu, |menu, item| {
        if flag(item, "separator") {
            return menu.separator();
        }
        let label = item
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let icon = item
            .get("icon")
            .and_then(Value::as_str)
            .map(|name| Icon::default().path(icon_path(name)));
        let disabled = flag(item, "disabled");
        match item.get("items").and_then(Value::as_array) {
            // gpui-component cannot disable a submenu, so a disabled one is drawn as a disabled item.
            Some(sub) if !disabled => {
                let (sub, session) = (sub.clone(), session.clone());
                menu.submenu_with_icon(icon, label, window, cx, move |menu, window, cx| {
                    build(menu, &sub, &session, id, window, cx)
                })
            }
            _ => {
                let item_id = item
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                let session = session.clone();
                let shortcut = item
                    .get("shortcut")
                    .and_then(Value::as_str)
                    .and_then(crate::keys::parse);
                // A shortcut is drawn at the right of the label, as gpui-component does for actions.
                let mut entry = match shortcut {
                    Some(keystroke) => PopupMenuItem::element(move |_, _| {
                        h_flex()
                            .w_full()
                            .justify_between()
                            .gap_4()
                            .child(label.clone())
                            .child(Kbd::new(keystroke.clone()).appearance(false))
                    }),
                    None => PopupMenuItem::new(label),
                };
                entry = entry.disabled(disabled).checked(flag(item, "checked"));
                if let Some(icon) = icon {
                    entry = entry.icon(icon);
                }
                menu.item(
                    entry.on_click(move |_, _, cx| chosen(&session.read(cx).tree, id, &item_id)),
                )
            }
        }
    })
}

/// Fills a gpui-component `PopupMenu` (a context menu or a MenuBar menu).
pub type Builder = Box<dyn Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu>;

/// Builder for `context_menu`/`dropdown_menu` from a node's item list.
pub fn builder(items: Vec<Value>, session: &Entity<Session>, id: NodeId) -> Builder {
    let session = session.clone();
    Box::new(move |menu, window, cx| build(menu, &items, &session, id, window, cx))
}

/// A `MenuBar`: one ghost button per menu, each opening its items.
pub fn bar(id: NodeId, menus: &[Value], session: &Entity<Session>) -> Div {
    div()
        .flex()
        .items_center()
        .children(menus.iter().enumerate().map(|(ix, menu)| {
            let label = menu
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let items = menu
                .get("items")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            Button::new(SharedString::from(format!("menubar-{id}-{ix}")))
                .ghost()
                .small()
                .label(label)
                .dropdown_menu(builder(items, session, id))
            // shortcut: no hover switching or arrow keys between menus (AppMenuBar has them but needs GPUI actions).
        }))
}
