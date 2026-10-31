//! `ColorPicker` (PROTOCOL.md section 25): one retained `ColorPickerState` per node, drawn as the
//! crate's `ColorSelect` (the framed field that sits in a form like a Select). The chosen color
//! lives in the tree (`value`, `#RRGGBB` or `#RRGGBBAA`), and `sync` makes the state show it.
//! `palette` lists featured colors. The crate cannot disable either of its color pickers.

use std::collections::{HashMap, HashSet};

use gpui_kit::component::Colorize as _;
use gpui_kit::component::color_picker::{ColorPickerEvent, ColorPickerState, ColorSelect};
use gpui_kit::*;
use serde_json::Value;

use super::{prop_str, value_chosen};
use crate::Session;
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree};

pub struct ColorPickerSlot {
    state: Entity<ColorPickerState>,
    _changes: Subscription,
}

/// Each ColorPicker among `members`: the color to show, as the tree has it.
pub struct ColorPickerSnapshot(Vec<(NodeId, Option<String>)>);

/// `#RRGGBB` or `#RRGGBBAA`: the `#` is required, and only ASCII reaches the crate's parser,
/// which slices by bytes.
pub fn parse(value: &str) -> Option<Hsla> {
    let digits = value.strip_prefix('#')?;
    let ok = matches!(digits.len(), 6 | 8) && digits.bytes().all(|b| b.is_ascii_hexdigit());
    ok.then(|| Hsla::parse_hex(digits).ok()).flatten()
}

pub fn snapshot(tree: &Tree, members: &HashSet<NodeId>) -> ColorPickerSnapshot {
    ColorPickerSnapshot(
        tree.nodes()
            .filter(|(id, node)| node.node_type == NodeType::ColorPicker && members.contains(id))
            .map(|(id, node)| (id, prop_str(node, "value").map(str::to_owned)))
            .collect(),
    )
}

/// Creates states, drops removed ones, and shows each node's color (which also reverts a
/// controlled ColorPicker until the client confirms it).
pub fn sync<V: 'static>(
    slots: &mut HashMap<NodeId, ColorPickerSlot>,
    snapshot: ColorPickerSnapshot,
    session: &Entity<Session>,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    slots.retain(|id, _| snapshot.0.iter().any(|(picker, _)| picker == id));
    for (id, value) in snapshot.0 {
        let slot = slots.entry(id).or_insert_with(|| {
            let state = cx.new(|cx| ColorPickerState::new(window, cx));
            let session = session.clone();
            let _changes = cx.subscribe_in(
                &state,
                window,
                move |_, _, event: &ColorPickerEvent, _, cx| {
                    let ColorPickerEvent::Change(chosen) = event;
                    let value = chosen.map(|color| color.to_hex());
                    session.update(cx, |session, cx| {
                        let current = session
                            .tree
                            .node(id)
                            .and_then(|node| prop_str(node, "value"));
                        let same = match (current, &value) {
                            (Some(current), Some(value)) => current.eq_ignore_ascii_case(value),
                            (current, value) => current.is_none() && value.is_none(),
                        };
                        if !same {
                            value_chosen(
                                &mut session.tree,
                                id,
                                value.map_or(Value::Null, Value::String),
                            );
                        }
                        // Synced or not, re-sync: a controlled ColorPicker goes back to the client's color.
                        cx.notify();
                    });
                },
            );
            ColorPickerSlot { state, _changes }
        });
        // Compared as hex, not as `Hsla`: a slider drag does not survive the trip through the tree
        // exactly, and `sync` runs on every render.
        let shown = slot.state.read(cx).value().map(|color| color.to_hex());
        let wanted = value.as_deref().and_then(parse);
        let same = match (&shown, value.as_deref()) {
            (Some(shown), Some(value)) => {
                shown.eq_ignore_ascii_case(value) || wanted.is_some_and(|c| c.to_hex() == *shown)
            }
            (shown, _) => shown.is_none() && wanted.is_none(),
        };
        if !same {
            slot.state.update(cx, |state, cx| match wanted {
                Some(color) => state.set_value(color, window, cx),
                None => state.clear_value(window, cx),
            });
        }
    }
}

/// Props: `value` (applied by `sync`), `palette`, `controlled`.
pub fn build(node: &Node, slot: &ColorPickerSlot) -> ColorSelect {
    let palette: Vec<Hsla> = node
        .props
        .get("palette")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|color| color.as_str().and_then(parse))
        .collect();
    let picker = ColorSelect::new(&slot.state);
    if palette.is_empty() {
        picker
    } else {
        picker.featured_colors(palette)
    }
}
