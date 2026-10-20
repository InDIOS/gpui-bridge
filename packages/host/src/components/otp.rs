//! `OtpInput` (PROTOCOL.md section 17): one `OtpState` per node. The code lives in the host,
//! like an `Input`'s text, and `sync` applies the `value` the client imposes.

use std::collections::{HashMap, HashSet};

use gpui_kit::component::Disableable;
use gpui_kit::component::input::{OtpEvent, OtpInput, OtpState};
use gpui_kit::*;

use super::{input_edited, prop_bool, prop_str};
use crate::Session;
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree};

pub struct OtpSlot {
    state: Entity<OtpState>,
    length: usize,
    _changes: Subscription,
}

impl OtpSlot {
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.state.update(cx, |state, cx| state.focus(window, cx));
    }
}

/// Each OtpInput among `members`: length, value to show and whether it is masked.
pub struct OtpSnapshot(Vec<(NodeId, usize, String, bool)>);

pub fn snapshot(tree: &Tree, members: &HashSet<NodeId>) -> OtpSnapshot {
    OtpSnapshot(
        tree.nodes()
            .filter(|(id, node)| node.node_type == NodeType::OtpInput && members.contains(id))
            .map(|(id, node)| {
                let length = node
                    .props
                    .get("length")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(6)
                    .clamp(1, 32) as usize;
                let value = prop_str(node, "value").unwrap_or("").to_owned();
                let masked = node.props.get("masked") == Some(&serde_json::Value::Bool(true));
                (id, length, value, masked)
            })
            .collect(),
    )
}

/// Creates states, drops removed ones, and shows each node's value. A new `length` needs a new
/// state: the fixed length is given when the state is created.
pub fn sync<V: 'static>(
    slots: &mut HashMap<NodeId, OtpSlot>,
    snapshot: OtpSnapshot,
    session: &Entity<Session>,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    slots.retain(|id, _| snapshot.0.iter().any(|(otp, ..)| otp == id));
    for (id, length, value, masked) in snapshot.0 {
        if slots.get(&id).is_none_or(|slot| slot.length != length) {
            let state = cx.new(|cx| OtpState::new(length, window, cx));
            let session = session.clone();
            let _changes = cx.subscribe_in(
                &state,
                window,
                move |_, state, event: &OtpEvent, window, cx| {
                    if !matches!(event, OtpEvent::Change | OtpEvent::Complete) {
                        return;
                    }
                    let code = state.read(cx).value().to_string();
                    let revert = input_edited(&session.read(cx).tree, id, &code);
                    // Controlled: back to the client's value until it accepts the code.
                    if let Some(accepted) = revert {
                        state.update(cx, |state, cx| state.set_value(accepted, window, cx));
                    }
                },
            );
            slots.insert(
                id,
                OtpSlot {
                    state,
                    length,
                    _changes,
                },
            );
        }
        let slot = &slots[&id];
        if slot.state.read(cx).value().as_ref() != value {
            slot.state
                .update(cx, |state, cx| state.set_value(value.clone(), window, cx));
        }
        if slot.state.read(cx).is_masked() != masked {
            slot.state
                .update(cx, |state, cx| state.set_masked(masked, window, cx));
        }
    }
}

/// Props: `length`, `value` (applied by `sync`), `masked`, `disabled`, `controlled`.
pub fn build(node: &Node, slot: &OtpSlot) -> OtpInput {
    OtpInput::new(&slot.state).disabled(prop_bool(node, "disabled"))
}
