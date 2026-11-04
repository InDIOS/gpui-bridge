//! The system tray icon (PROTOCOL.md section 11), with `tray-icon`: GPUI has none. Its menu is a
//! native Windows menu built from the same item list as `contextMenu`.

use std::cell::RefCell;

use gpui_kit::{App, Entity};
use serde_json::Value;
use tray_icon::menu::{
    CheckMenuItem, ContextMenu, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::Session;
use crate::components::menu::flag;
use crate::protocol::HostMessage;
use crate::transport;

/// The tray icon shown, and the `TraySpec` revision it shows.
#[derive(Default)]
pub struct Tray {
    icon: Option<TrayIcon>,
    revision: u64,
}

/// Reports clicks and menu choices. The handlers run on the main thread, from GPUI's message loop.
pub fn init() {
    TrayIconEvent::set_event_handler(Some(|event| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            transport::send(&HostMessage::Tray {
                event: "click".into(),
                value: None,
            });
        }
    }));
    MenuEvent::set_event_handler(Some(|event: MenuEvent| {
        transport::send(&HostMessage::Tray {
            event: "menu".into(),
            value: Some(event.id.0),
        });
    }));
}

/// Applies the committed tray: creates, updates (on every `tray` message) or removes the icon.
pub fn sync(session: &Entity<Session>, tray: &RefCell<Tray>, cx: &App) {
    let spec = session.read(cx).tree.tray().cloned();
    let mut tray = tray.borrow_mut();
    let Some(spec) = spec else {
        // Dropping the icon removes it from the tray.
        *tray = Tray::default();
        return;
    };
    if spec.revision == tray.revision {
        return;
    }
    tray.revision = spec.revision;
    let icon = spec.icon.as_deref().and_then(load_icon);
    let menu = spec
        .menu
        .as_ref()
        .map(|items| Box::new(build_menu(items)) as Box<dyn ContextMenu>);
    match &tray.icon {
        Some(shown) => {
            log_err(shown.set_icon(icon));
            log_err(shown.set_tooltip(spec.tooltip.as_deref()));
            shown.set_menu(menu);
        }
        None => {
            let mut builder = TrayIconBuilder::new().with_menu_on_left_click(false);
            if let Some(icon) = icon {
                builder = builder.with_icon(icon);
            }
            if let Some(tooltip) = &spec.tooltip {
                builder = builder.with_tooltip(tooltip);
            }
            if let Some(menu) = menu {
                builder = builder.with_menu(menu);
            }
            match builder.build() {
                Ok(icon) => tray.icon = Some(icon),
                Err(err) => log::error!("failed to create the tray icon: {err}"),
            }
        }
    }
}

fn log_err<E: std::fmt::Display>(result: Result<(), E>) {
    if let Err(err) = result {
        log::warn!("failed to update the tray icon: {err}");
    }
}

/// A missing or undecodable file leaves the icon empty, with a warning (like `Image`).
fn load_icon(path: &str) -> Option<Icon> {
    let image = image::open(path)
        .map_err(|err| log::warn!("Tray icon {path}: {err}"))
        .ok()?;
    // shortcut: one 32 px bitmap that Windows scales to the tray size; pass several sizes if it looks soft at high DPI.
    let image = image.thumbnail(32, 32).into_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height)
        .map_err(|err| log::warn!("Tray icon {path}: {err}"))
        .ok()
}

fn build_menu(items: &Value) -> Menu {
    let menu = Menu::new();
    for item in menu_items(items) {
        log_err(menu.append(item.as_ref()));
    }
    menu
}

/// Validated items (`menu::check_items`) as native ones. Item icons are not drawn.
fn menu_items(items: &Value) -> Vec<Box<dyn IsMenuItem>> {
    let items = items.as_array().map_or(&[][..], Vec::as_slice);
    items
        .iter()
        .map(|item| -> Box<dyn IsMenuItem> {
            if flag(item, "separator") {
                return Box::new(PredefinedMenuItem::separator());
            }
            // `&` marks a keyboard mnemonic in Windows menus; `&&` draws it.
            let label = item
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("")
                .replace('&', "&&");
            let enabled = !flag(item, "disabled");
            if let Some(sub) = item.get("items") {
                let submenu = Submenu::new(label, enabled);
                for child in menu_items(sub) {
                    log_err(submenu.append(child.as_ref()));
                }
                return Box::new(submenu);
            }
            let id = item.get("id").and_then(Value::as_str).unwrap_or("");
            // Only shown: a tray menu exists only while it is open.
            let accelerator = item
                .get("shortcut")
                .and_then(Value::as_str)
                .and_then(|keys| keys.replace('-', "+").parse().ok());
            match item.get("checked").and_then(Value::as_bool) {
                Some(checked) => Box::new(CheckMenuItem::with_id(
                    id,
                    label,
                    enabled,
                    checked,
                    accelerator,
                )),
                None => Box::new(MenuItem::with_id(id, label, enabled, accelerator)),
            }
        })
        .collect()
}
