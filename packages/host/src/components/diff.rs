//! `Diff` (PROTOCOL.md section 18): one retained `DiffState` per node, rebuilt only when the
//! `patch` changes. The patch is parsed when the props are validated, so a malformed one is
//! `invalid_props` instead of an empty view.

use std::collections::{HashMap, HashSet};

use gpui_kit::component::diff::{Diff, DiffEvent, DiffFile, DiffMode, DiffState};
use gpui_kit::*;
use serde_json::{Value, json};

use crate::Session;
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree};

pub struct DiffSlot {
    state: Entity<DiffState>,
    /// The patch the state was built from.
    patch: String,
    mode: DiffMode,
    _events: Subscription,
}

/// Each Diff among `members`: its patch and display mode.
pub struct DiffSnapshot(Vec<(NodeId, String, DiffMode)>);

/// A `mode` prop; anything else is the component's default (unified).
pub fn mode(node: &Node) -> DiffMode {
    match node.props.get("mode").and_then(Value::as_str) {
        Some("split") => DiffMode::Split,
        _ => DiffMode::Unified,
    }
}

/// Checks `patch`: a unified or Git diff that gpui-component can parse.
pub fn check_patch(value: &Value) -> Result<(), String> {
    let patch = value.as_str().ok_or("a patch string")?;
    DiffFile::parse(patch)
        .map(|_| ())
        .map_err(|err| format!("a unified diff ({err})"))
}

pub fn snapshot(tree: &Tree, members: &HashSet<NodeId>) -> DiffSnapshot {
    DiffSnapshot(
        tree.nodes()
            .filter(|(id, node)| node.node_type == NodeType::Diff && members.contains(id))
            .map(|(id, node)| {
                let patch = node
                    .props
                    .get("patch")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                (id, patch, mode(node))
            })
            .collect(),
    )
}

/// The `change` payload of a selected range.
fn selection(range: &gpui_kit::component::diff::DiffLineRange) -> Value {
    let side = |side: gpui_kit::component::diff::DiffSide| match side {
        gpui_kit::component::diff::DiffSide::Original => "original",
        gpui_kit::component::diff::DiffSide::Modified => "modified",
    };
    json!({
        "path": range.path().to_string(),
        "side": side(range.side()),
        "start": range.start(),
        "end": range.end(),
        "endSide": side(range.end_side()),
    })
}

/// Reports a selected range from `simulate` (`None`: the selection was cleared).
pub fn selection_reported(tree: &Tree, id: NodeId, value: Option<Value>) {
    super::emit(tree, id, "change", Some(value.unwrap_or(Value::Null)), None);
}

/// Creates state, drops removed ones, and rebuilds a node whose patch changed. A patch the
/// validator rejected never reaches here; if it somehow does, the old files stay.
pub fn sync<V: 'static>(
    slots: &mut HashMap<NodeId, DiffSlot>,
    snapshot: DiffSnapshot,
    session: &Entity<Session>,
    cx: &mut Context<V>,
) {
    slots.retain(|id, _| snapshot.0.iter().any(|(diff, ..)| diff == id));
    for (id, patch, mode) in snapshot.0 {
        let stale = slots
            .get(&id)
            .is_none_or(|slot| slot.patch != patch || slot.mode != mode);
        if stale {
            let Ok(files) = DiffFile::parse(&patch) else {
                continue;
            };
            match slots.get(&id) {
                Some(slot) => {
                    let state = slot.state.clone();
                    state.update(cx, |state, cx| {
                        state.set_files(files, cx);
                        state.set_mode(mode, cx);
                    });
                    if let Some(slot) = slots.get_mut(&id) {
                        slot.patch = patch;
                        slot.mode = mode;
                    }
                }
                None => {
                    let state = cx.new(|cx| {
                        let mut state = DiffState::new(files, cx);
                        state.set_mode(mode, cx);
                        state
                    });
                    let session = session.clone();
                    let events = cx.subscribe(&state, move |_, _, event: &DiffEvent, cx| {
                        let DiffEvent::SelectionChanged(range) = event else {
                            return;
                        };
                        let value = Some(range.as_ref().map(selection).unwrap_or(Value::Null));
                        session.update(cx, |session, _| {
                            super::emit(&session.tree, id, "change", value, None);
                        });
                    });
                    slots.insert(
                        id,
                        DiffSlot {
                            state,
                            patch,
                            mode,
                            _events: events,
                        },
                    );
                }
            }
        }
    }
}

/// Props: `patch` and `mode` (applied by `sync`), `lineNumbers` (default true), `softWrap`
/// (default false) and `header` (default true). Its `change` carries `{path, side, start, end,
/// endSide}`, or `null` when the selection is cleared.
pub fn build(node: &Node, slot: &DiffSlot) -> Diff {
    let flag = |key, default| {
        node.props
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(default)
    };
    Diff::new(&slot.state)
        .line_number(flag("lineNumbers", true))
        .soft_wrap(flag("softWrap", false))
        .header_visible(flag("header", true))
}

#[cfg(test)]
mod tests {
    // Not `super::*`: gpui's own `test` attribute would shadow the standard one.
    use super::check_patch;
    use serde_json::json;

    const PATCH: &str = "diff --git a/notes.txt b/notes.txt\n--- a/notes.txt\n+++ b/notes.txt\n@@ -1,3 +1,4 @@\n one\n-two\n+2\n three\n+four\n";

    #[test]
    fn a_patch_is_parsed_when_the_prop_is_checked() {
        assert!(check_patch(&json!(PATCH)).is_ok());
        assert!(check_patch(&json!("")).is_ok());
        assert!(check_patch(&json!("not a diff")).is_err());
        assert!(check_patch(&json!(12)).is_err());
    }
}
