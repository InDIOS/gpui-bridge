//! Keyboard shortcuts (PROTOCOL.md section 12): app-wide `shortcuts` and `shortcut` on menu
//! items. Shared by the window's key listener and `simulate`.

use gpui_kit::{Keystroke, OperatingSystem};
use serde_json::Value;

use crate::components::menu;
use crate::protocol::{HostMessage, WindowId};
use crate::schema::NodeType;
use crate::transport;
use crate::tree::Tree;

/// The OS whose modifier names GPUI parses and writes (`cmd` on macOS, `win` elsewhere).
pub const OS: OperatingSystem = if cfg!(target_os = "macos") {
    OperatingSystem::Mac
} else if cfg!(target_os = "windows") {
    OperatingSystem::Windows
} else {
    OperatingSystem::Linux
};

/// A key in GPUI's format (`ctrl-shift-s`, `escape`, `f5`), or `None` if it does not parse.
pub fn parse(keys: &str) -> Option<Keystroke> {
    Keystroke::parse_with_os(keys, OS)
        .ok()
        .filter(|keystroke| !keystroke.key.is_empty())
}

/// Same key and the same modifiers, nothing more or less.
pub fn matches(pressed: &Keystroke, target: &Keystroke) -> bool {
    pressed.modifiers == target.modifiers && pressed.key == target.key
}

/// A key pressed in window `window`: reports a registered shortcut, or chooses the enabled
/// `MenuBar` item with that `shortcut` in the window. Returns whether it was used, so the
/// focused control does not get it too.
pub fn pressed(tree: &Tree, window: WindowId, pressed: &Keystroke) -> bool {
    if let Some(keys) = tree
        .shortcuts()
        .iter()
        .find(|keys| parse(keys).is_some_and(|target| matches(pressed, &target)))
    {
        transport::send(&HostMessage::Shortcut {
            keys: keys.clone(),
            window,
        });
        return true;
    }
    let Some(root) = tree.windows().get(&window).and_then(|spec| spec.root) else {
        return false;
    };
    for id in tree.subtree(root) {
        let Some(node) = tree
            .node(id)
            .filter(|node| node.node_type == NodeType::MenuBar)
        else {
            continue;
        };
        let menus = node
            .props
            .get("menus")
            .and_then(Value::as_array)
            .into_iter()
            .flatten();
        if let Some(item) = menus
            .filter_map(|m| m.get("items"))
            .find_map(|items| menu::find_shortcut(items, pressed))
        {
            menu::chosen(tree, id, &item);
            return true;
        }
    }
    false
}
